use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use openpics_core::{DocId, Document, LayerId, TILE_SIZE, Tile, TileKey, ViewSettings, Viewport};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::error::GpuError;

const SHADER: &str = include_str!("shader.wgsl");
const HEADLESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Plancher du cache de textures : de quoi couvrir un grand écran à 100 % sans rechargement.
const MIN_CACHED_TILES: usize = 1024;
/// Sous cette largeur en pixels d'écran, une ligne de grille n'est plus lisible : on l'omet.
const MIN_GRID_PX: f64 = 6.0;
const MAX_GRID_LINES: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    fn canvas_background(self) -> wgpu::Color {
        match self {
            Theme::Light => wgpu::Color { r: 0.894, g: 0.906, b: 0.933, a: 1.0 },
            Theme::Dark => wgpu::Color { r: 0.071, g: 0.094, b: 0.149, a: 1.0 },
        }
    }

    fn checker(self) -> ([f32; 4], [f32; 4]) {
        match self {
            Theme::Light => ([1.0, 1.0, 1.0, 1.0], [0.80, 0.80, 0.80, 1.0]),
            Theme::Dark => ([0.34, 0.34, 0.34, 1.0], [0.24, 0.24, 0.24, 1.0]),
        }
    }

    fn border(self) -> [f32; 4] {
        match self {
            Theme::Light => [0.0, 0.0, 0.0, 0.35],
            Theme::Dark => [1.0, 1.0, 1.0, 0.30],
        }
    }
}

/// Ce qu'il faut dessiner pour une image. `viewport` est en pixels physiques de la surface.
pub struct FrameParams<'a> {
    pub document: Option<&'a Document>,
    pub viewport: Viewport,
    pub settings: ViewSettings,
    pub theme: Theme,
    /// Repère en cours de création (glissé depuis une règle), non encore dans le document.
    pub preview_guide: Option<(openpics_core::Axis, f64)>,
}

/// Poignées natives de la fenêtre qui reçoit le canvas.
pub struct WindowTarget {
    pub window: RawWindowHandle,
    pub display: RawDisplayHandle,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    screen: [f32; 2],
    _pad: [f32; 2],
    checker_a: [f32; 4],
    checker_b: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    rect: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LineVertex {
    pos: [f32; 2],
    color: [f32; 4],
}

/// Ce qui a produit la texture : sert à savoir si elle est encore à jour.
enum Source {
    /// Tuile d'un calque unique : une tuile modifiée est une autre `Arc`.
    Tile(Arc<Tile>),
    /// Tuile composée par le CPU : valable tant que la révision du document ne change pas.
    Revision(u64),
}

struct CachedTile {
    source: Source,
    bind_group: wgpu::BindGroup,
    last_used: u64,
}

type CacheKey = (DocId, LayerId, TileKey);

enum Draw {
    Checker,
    Solid,
    Tile(CacheKey),
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    size: (u32, u32),
    globals_buf: wgpu::Buffer,
    globals_nearest: wgpu::BindGroup,
    globals_linear: wgpu::BindGroup,
    tile_layout: wgpu::BindGroupLayout,
    checker_pipeline: wgpu::RenderPipeline,
    solid_pipeline: wgpu::RenderPipeline,
    tile_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    instance_buf: wgpu::Buffer,
    line_buf: wgpu::Buffer,
    cache: HashMap<CacheKey, CachedTile>,
    frame: u64,
}

impl Renderer {
    /// Moteur sans fenêtre : rendu dans une texture lue par `render_to_rgba8` (tests, export).
    pub fn new_headless() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = request_adapter(&instance, None)?;
        Self::build(&adapter, None, HEADLESS_FORMAT, (1, 1))
    }

    /// Moteur dessinant dans une fenêtre native.
    ///
    /// # Safety
    /// Les poignées de `target` doivent désigner une fenêtre valide qui reste en vie plus longtemps
    /// que le `Renderer`.
    pub unsafe fn new_for_window(target: WindowTarget, size: (u32, u32)) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        // SAFETY: garanti par l'appelant (voir la section Safety ci-dessus).
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(target.display),
                raw_window_handle: target.window,
            })
        }
        .map_err(|e| GpuError::Surface(e.to_string()))?;
        let adapter = request_adapter(&instance, Some(&surface))?;
        let caps = surface.get_capabilities(&adapter);
        // Format sans conversion sRGB : les octets du document (déjà sRGB) sont affichés tels quels.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| GpuError::Surface("aucun format de surface disponible".into()))?;
        Self::build(&adapter, Some(surface), format, size)
    }

    fn build(
        adapter: &wgpu::Adapter,
        surface: Option<wgpu::Surface<'static>>,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> Result<Self, GpuError> {
        let (device, queue) = pollster::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor { label: Some("openpics"), ..Default::default() }),
        )
        .map_err(|e| GpuError::Device(e.to_string()))?;

        let surface_config = match &surface {
            Some(s) => {
                let mut cfg = s
                    .get_default_config(adapter, size.0.max(1), size.1.max(1))
                    .ok_or_else(|| GpuError::Surface("surface incompatible avec la carte graphique".into()))?;
                cfg.format = format;
                // Fifo : pas de déchirure, et 1 image de latence pour que le pan suive la souris.
                cfg.present_mode = wgpu::PresentMode::Fifo;
                cfg.desired_maximum_frame_latency = 1;
                s.configure(&device, &cfg);
                Some(cfg)
            }
            None => None,
        };

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("openpics canvas"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let tile_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tile"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let make_sampler = |filter: wgpu::FilterMode| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: filter,
                min_filter: filter,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            })
        };
        let make_globals = |sampler: &wgpu::Sampler| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("globals"),
                layout: &globals_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            })
        };
        // Pixels francs au-dessus de 100 % (on voit les pixels), lissage en dessous.
        let globals_nearest = make_globals(&make_sampler(wgpu::FilterMode::Nearest));
        let globals_linear = make_globals(&make_sampler(wgpu::FilterMode::Linear));

        let quad_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let tile_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tile"),
            bind_group_layouts: &[Some(&globals_layout), Some(&tile_layout)],
            immediate_size: 0,
        });

        let instance_attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance_attrs,
        };
        let line_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
        let line_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &line_attrs,
        };
        let premultiplied = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let make_pipeline = |label: &str,
                             layout: &wgpu::PipelineLayout,
                             vs: &str,
                             fs: &str,
                             buffer: wgpu::VertexBufferLayout<'_>,
                             topology: wgpu::PrimitiveTopology,
                             blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[Some(buffer)],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let tri = wgpu::PrimitiveTopology::TriangleList;
        let checker_pipeline =
            make_pipeline("checker", &quad_layout, "vs_quad", "fs_checker", instance_layout.clone(), tri, None);
        let solid_pipeline = make_pipeline(
            "solid",
            &quad_layout,
            "vs_quad",
            "fs_solid",
            instance_layout.clone(),
            tri,
            Some(premultiplied),
        );
        let tile_pipeline = make_pipeline(
            "tile",
            &tile_pipeline_layout,
            "vs_quad",
            "fs_tile",
            instance_layout,
            tri,
            Some(premultiplied),
        );
        let line_pipeline = make_pipeline(
            "line",
            &quad_layout,
            "vs_line",
            "fs_line",
            line_layout,
            wgpu::PrimitiveTopology::LineList,
            Some(premultiplied),
        );

        let instance_buf = dynamic_buffer(&device, "instances", 4096, wgpu::BufferUsages::VERTEX);
        let line_buf = dynamic_buffer(&device, "lines", 4096, wgpu::BufferUsages::VERTEX);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            size,
            globals_buf,
            globals_nearest,
            globals_linear,
            tile_layout,
            checker_pipeline,
            solid_pipeline,
            tile_pipeline,
            line_pipeline,
            instance_buf,
            line_buf,
            cache: HashMap::new(),
            frame: 0,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Ignore les tailles nulles (fenêtre réduite) : `configure` les refuse.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.size = (width, height);
        if let (Some(surface), Some(cfg)) = (&self.surface, &mut self.surface_config) {
            cfg.width = width;
            cfg.height = height;
            surface.configure(&self.device, cfg);
        }
    }

    /// Dessine une image dans la surface. Une surface perdue ou obsolète saute l'image au lieu
    /// d'échouer : elle est reconfigurée pour la suivante.
    pub fn render(&mut self, frame: &FrameParams<'_>) -> Result<(), GpuError> {
        let surface = self.surface.as_ref().ok_or(GpuError::NoSurface)?;
        let texture = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let (w, h) = self.size;
                self.resize(w, h);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let size = self.size;
        self.draw(frame, &view, size);
        self.queue.present(texture);
        Ok(())
    }

    /// Rendu hors écran en RGBA 8 bits, lignes contiguës.
    pub fn render_to_rgba8(&mut self, frame: &FrameParams<'_>, size: (u32, u32)) -> Result<Vec<u8>, GpuError> {
        let (w, h) = size;
        if w == 0 || h == 0 || w > 16_384 || h > 16_384 {
            return Err(GpuError::InvalidSize { width: w, height: h });
        }
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HEADLESS_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        self.draw(frame, &view, size);

        let row_bytes = w * 4;
        let padded = row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| GpuError::Readback(e.to_string()))?;
        rx.recv().map_err(|e| GpuError::Readback(e.to_string()))?.map_err(|e| GpuError::Readback(e.to_string()))?;
        let mapped = buffer.slice(..).get_mapped_range().map_err(|e| GpuError::Readback(e.to_string()))?;
        let mut out = Vec::with_capacity((row_bytes * h) as usize);
        for row in mapped.chunks_exact(padded as usize) {
            out.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(out)
    }

    fn draw(&mut self, frame: &FrameParams<'_>, target: &wgpu::TextureView, size: (u32, u32)) {
        self.frame += 1;
        let (sw, sh) = (size.0 as f32, size.1 as f32);
        let (checker_a, checker_b) = frame.theme.checker();
        self.queue.write_buffer(
            &self.globals_buf,
            0,
            bytemuck::bytes_of(&Globals { screen: [sw, sh], _pad: [0.0; 2], checker_a, checker_b }),
        );

        let mut instances: Vec<Instance> = Vec::new();
        let mut draws: Vec<Draw> = Vec::new();
        let mut doc_scissor: Option<(u32, u32, u32, u32)> = None;

        if let Some(doc) = frame.document {
            let zoom = frame.viewport.zoom;
            let dsize = doc.size();
            let tl = frame.viewport.doc_to_screen(openpics_core::Vec2::new(0.0, 0.0));
            let br =
                frame.viewport.doc_to_screen(openpics_core::Vec2::new(f64::from(dsize.width), f64::from(dsize.height)));
            doc_scissor = clip_rect(tl.x, tl.y, br.x, br.y, size);
            if doc_scissor.is_some() {
                instances
                    .push(Instance { rect: [tl.x as f32, tl.y as f32, br.x as f32, br.y as f32], color: [0.0; 4] });
                draws.push(Draw::Checker);
                let visible = frame.viewport.visible_doc_rect(openpics_core::Size::new(size.0, size.1));
                let plain = is_plain(doc);
                if !plain {
                    // Modes de fusion, opacité, masques, écrêtage : compositeur de référence (CPU), une
                    // tuile composée à la fois, recalculée seulement quand le document change.
                    let ts = f64::from(TILE_SIZE);
                    let sample = doc.layers.first().map(|l| &l.pixels);
                    for key in sample.map(|g| g.keys_in_rect(visible)).unwrap_or_default() {
                        if doc.layers.iter().all(|l| l.pixels.tile(key).is_none()) {
                            continue;
                        }
                        let a = frame
                            .viewport
                            .doc_to_screen(openpics_core::Vec2::new(f64::from(key.tx) * ts, f64::from(key.ty) * ts));
                        let b = frame.viewport.doc_to_screen(openpics_core::Vec2::new(
                            f64::from(key.tx + 1) * ts,
                            f64::from(key.ty + 1) * ts,
                        ));
                        let ck = (doc.id, LayerId(0), key);
                        self.ensure_composed(ck, doc, key);
                        instances.push(Instance {
                            rect: [a.x.round() as f32, a.y.round() as f32, b.x.round() as f32, b.y.round() as f32],
                            color: [0.0; 4],
                        });
                        draws.push(Draw::Tile(ck));
                    }
                }
                for layer in doc.layers.iter().filter(|l| l.visible && plain) {
                    for key in layer.pixels.keys_in_rect(visible) {
                        let Some(tile) = layer.pixels.tile(key) else {
                            continue;
                        };
                        let ts = f64::from(TILE_SIZE);
                        let a = frame
                            .viewport
                            .doc_to_screen(openpics_core::Vec2::new(f64::from(key.tx) * ts, f64::from(key.ty) * ts));
                        let b = frame.viewport.doc_to_screen(openpics_core::Vec2::new(
                            f64::from(key.tx + 1) * ts,
                            f64::from(key.ty + 1) * ts,
                        ));
                        // Bords arrondis depuis la même formule pour deux tuiles voisines : pas de fente.
                        let rect = [a.x.round() as f32, a.y.round() as f32, b.x.round() as f32, b.y.round() as f32];
                        match tile.as_ref() {
                            Tile::Uniform(c) => {
                                let al = f32::from(c[3]) / 255.0;
                                instances.push(Instance {
                                    rect,
                                    color: [
                                        f32::from(c[0]) / 255.0 * al,
                                        f32::from(c[1]) / 255.0 * al,
                                        f32::from(c[2]) / 255.0 * al,
                                        al,
                                    ],
                                });
                                draws.push(Draw::Solid);
                            }
                            Tile::Pixels(_) => {
                                let ck = (doc.id, layer.id, key);
                                self.ensure_tile(ck, tile);
                                instances.push(Instance { rect, color: [0.0; 4] });
                                draws.push(Draw::Tile(ck));
                            }
                        }
                    }
                }
                let _ = zoom;
            }
        }

        let lines = build_lines(frame, size);
        write_dynamic(
            &self.device,
            &self.queue,
            &mut self.instance_buf,
            "instances",
            wgpu::BufferUsages::VERTEX,
            bytemuck::cast_slice(&instances),
        );
        write_dynamic(
            &self.device,
            &self.queue,
            &mut self.line_buf,
            "lines",
            wgpu::BufferUsages::VERTEX,
            bytemuck::cast_slice(&lines),
        );

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("canvas"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(frame.theme.canvas_background()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let globals = if frame.viewport.zoom >= 1.0 { &self.globals_nearest } else { &self.globals_linear };
            pass.set_bind_group(0, Some(globals), &[]);
            pass.set_vertex_buffer(0, self.instance_buf.slice(..));
            if let Some((x, y, w, h)) = doc_scissor {
                // Le scissor coupe les tuiles de bord qui dépassent du document.
                pass.set_scissor_rect(x, y, w, h);
                let mut current: Option<u8> = None;
                for (i, d) in draws.iter().enumerate() {
                    let i = i as u32;
                    match d {
                        Draw::Checker => {
                            pass.set_pipeline(&self.checker_pipeline);
                            current = Some(0);
                        }
                        Draw::Solid => {
                            if current != Some(1) {
                                pass.set_pipeline(&self.solid_pipeline);
                                current = Some(1);
                            }
                        }
                        Draw::Tile(key) => {
                            let Some(entry) = self.cache.get(key) else {
                                continue;
                            };
                            if current != Some(2) {
                                pass.set_pipeline(&self.tile_pipeline);
                                current = Some(2);
                            }
                            pass.set_bind_group(1, Some(&entry.bind_group), &[]);
                        }
                    }
                    pass.draw(0..6, i..i + 1);
                }
            }
            if !lines.is_empty() {
                pass.set_scissor_rect(0, 0, size.0, size.1);
                pass.set_pipeline(&self.line_pipeline);
                pass.set_vertex_buffer(0, self.line_buf.slice(..));
                pass.draw(0..lines.len() as u32, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.evict(draws.len());
    }

    /// Téléverse la tuile si elle est absente ou si le document en a changé le contenu.
    fn ensure_tile(&mut self, key: CacheKey, tile: &Arc<Tile>) {
        if let Some(entry) = self.cache.get_mut(&key) {
            if matches!(&entry.source, Source::Tile(t) if Arc::ptr_eq(t, tile)) {
                entry.last_used = self.frame;
                return;
            }
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tile"),
            size: wgpu::Extent3d { width: TILE_SIZE, height: TILE_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        if let Tile::Pixels(data) = tile.as_ref() {
            self.queue.write_texture(
                texture.as_image_copy(),
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(TILE_SIZE * 4),
                    rows_per_image: Some(TILE_SIZE),
                },
                wgpu::Extent3d { width: TILE_SIZE, height: TILE_SIZE, depth_or_array_layers: 1 },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tile"),
            layout: &self.tile_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });
        self.cache.insert(key, CachedTile { source: Source::Tile(tile.clone()), bind_group, last_used: self.frame });
    }

    /// Compose une tuile avec le compositeur de référence et la téléverse.
    fn ensure_composed(&mut self, key: CacheKey, doc: &Document, tile: TileKey) {
        if let Some(entry) = self.cache.get_mut(&key) {
            if matches!(entry.source, Source::Revision(r) if r == doc.revision()) {
                entry.last_used = self.frame;
                return;
            }
        }
        let composed = openpics_core::composite::composite_tile(doc, tile);
        let bytes: Vec<u8> = composed.iter().flat_map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)).collect();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tile composée"),
            size: wgpu::Extent3d { width: TILE_SIZE, height: TILE_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            texture.as_image_copy(),
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(TILE_SIZE * 4),
                rows_per_image: Some(TILE_SIZE),
            },
            wgpu::Extent3d { width: TILE_SIZE, height: TILE_SIZE, depth_or_array_layers: 1 },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tile composée"),
            layout: &self.tile_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });
        self.cache
            .insert(key, CachedTile { source: Source::Revision(doc.revision()), bind_group, last_used: self.frame });
    }

    /// Retire les tuiles les moins récemment vues quand le cache dépasse son plafond.
    fn evict(&mut self, drawn: usize) {
        let cap = MIN_CACHED_TILES.max(drawn * 2);
        if self.cache.len() <= cap {
            return;
        }
        let mut by_age: Vec<(u64, CacheKey)> = self.cache.iter().map(|(k, v)| (v.last_used, *k)).collect();
        by_age.sort_unstable_by_key(|(age, _)| *age);
        for (_, key) in by_age.into_iter().take(self.cache.len() - cap) {
            self.cache.remove(&key);
        }
    }
}

/// Un seul calque visible tel quel (normal, 100 %, sans masque) : ses tuiles se dessinent directement.
fn is_plain(doc: &Document) -> bool {
    match doc.layers.as_slice() {
        [l] => {
            l.visible
                && l.opacity >= 1.0
                && l.fill >= 1.0
                && l.blend == openpics_core::BlendMode::Normal
                && l.mask.is_none()
                && !l.clip_to_below
        }
        _ => false,
    }
}

fn request_adapter(instance: &wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Result<wgpu::Adapter, GpuError> {
    let attempt = |fallback: bool| {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            force_fallback_adapter: fallback,
            ..Default::default()
        }))
    };
    // Repli sur un adaptateur logiciel (WARP) : mieux qu'une application qui refuse de démarrer.
    attempt(false).or_else(|_| attempt(true)).map_err(|e| GpuError::NoAdapter(e.to_string()))
}

fn dynamic_buffer(device: &wgpu::Device, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn write_dynamic(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &mut wgpu::Buffer,
    label: &str,
    usage: wgpu::BufferUsages,
    data: &[u8],
) {
    if data.is_empty() {
        return;
    }
    if (data.len() as u64) > buf.size() {
        *buf = dynamic_buffer(device, label, (data.len() as u64).next_power_of_two(), usage);
    }
    queue.write_buffer(buf, 0, data);
}

/// Intersection d'un rectangle (pixels d'écran, flottants) avec la surface, en entiers.
fn clip_rect(x0: f64, y0: f64, x1: f64, y1: f64, size: (u32, u32)) -> Option<(u32, u32, u32, u32)> {
    let cx0 = x0.max(0.0).floor();
    let cy0 = y0.max(0.0).floor();
    let cx1 = x1.min(f64::from(size.0)).ceil();
    let cy1 = y1.min(f64::from(size.1)).ceil();
    if cx1 <= cx0 || cy1 <= cy0 {
        return None;
    }
    Some((cx0 as u32, cy0 as u32, (cx1 - cx0) as u32, (cy1 - cy0) as u32))
}

/// Centre du pixel : une ligne de 1 px tombe pile sur une rangée, sans flou.
fn px(v: f64) -> f32 {
    (v.floor() + 0.5) as f32
}

fn build_lines(frame: &FrameParams<'_>, size: (u32, u32)) -> Vec<LineVertex> {
    let Some(doc) = frame.document else {
        return Vec::new();
    };
    let vp = &frame.viewport;
    let dsize = doc.size();
    let (sw, sh) = (f64::from(size.0), f64::from(size.1));
    let tl = vp.doc_to_screen(openpics_core::Vec2::new(0.0, 0.0));
    let br = vp.doc_to_screen(openpics_core::Vec2::new(f64::from(dsize.width), f64::from(dsize.height)));
    let mut out = Vec::new();
    let mut line = |a: [f32; 2], b: [f32; 2], color: [f32; 4]| {
        out.push(LineVertex { pos: a, color });
        out.push(LineVertex { pos: b, color });
    };

    if frame.settings.show_grid && frame.settings.grid_spacing > 0 {
        let step = f64::from(frame.settings.grid_spacing);
        if step * vp.zoom >= MIN_GRID_PX {
            let color = [0.5, 0.5, 0.5, 0.35];
            let (y0, y1) = (tl.y.max(0.0), br.y.min(sh));
            let (x0, x1) = (tl.x.max(0.0), br.x.min(sw));
            let first_x = ((x0 - tl.x) / (step * vp.zoom)).ceil() as i64;
            let first_y = ((y0 - tl.y) / (step * vp.zoom)).ceil() as i64;
            let mut count = 0;
            let mut i = first_x;
            while count < MAX_GRID_LINES {
                let x = tl.x + i as f64 * step * vp.zoom;
                if x > x1 {
                    break;
                }
                line([px(x), y0 as f32], [px(x), y1 as f32], color);
                i += 1;
                count += 1;
            }
            let mut j = first_y;
            while count < MAX_GRID_LINES {
                let y = tl.y + j as f64 * step * vp.zoom;
                if y > y1 {
                    break;
                }
                line([x0 as f32, px(y)], [x1 as f32, px(y)], color);
                j += 1;
                count += 1;
            }
        }
    }

    if frame.settings.show_guides {
        let color = [0.0, 0.72, 0.85, 1.0];
        for g in &doc.guides {
            match g.axis {
                openpics_core::Axis::Vertical => {
                    let x = vp.doc_to_screen(openpics_core::Vec2::new(g.position, 0.0)).x;
                    line([px(x), 0.0], [px(x), sh as f32], color);
                }
                openpics_core::Axis::Horizontal => {
                    let y = vp.doc_to_screen(openpics_core::Vec2::new(0.0, g.position)).y;
                    line([0.0, px(y)], [sw as f32, px(y)], color);
                }
            }
        }
    }

    if let Some((axis, pos)) = frame.preview_guide {
        let color = [0.0, 0.72, 0.85, 0.7];
        match axis {
            openpics_core::Axis::Vertical => {
                let x = vp.doc_to_screen(openpics_core::Vec2::new(pos, 0.0)).x;
                line([px(x), 0.0], [px(x), sh as f32], color);
            }
            openpics_core::Axis::Horizontal => {
                let y = vp.doc_to_screen(openpics_core::Vec2::new(0.0, pos)).y;
                line([0.0, px(y)], [sw as f32, px(y)], color);
            }
        }
    }

    // Contour du document, dessiné à l'extérieur pour ne pas mordre sur les pixels de l'image.
    let border = frame.theme.border();
    let (l, t, r, b) = (px(tl.x - 1.0), px(tl.y - 1.0), px(br.x), px(br.y));
    line([l, t], [r, t], border);
    line([r, t], [r, b], border);
    line([r, b], [l, b], border);
    line([l, b], [l, t], border);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::{DocId, Size, Vec2};

    #[test]
    fn shader_is_valid_wgsl() {
        let module = naga::front::wgsl::parse_str(SHADER).expect("le WGSL doit se parser");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .expect("le WGSL doit être valide");
    }

    #[test]
    fn clip_rect_cases() {
        assert_eq!(clip_rect(-10.0, -10.0, 50.5, 20.0, (100, 100)), Some((0, 0, 51, 20)));
        assert_eq!(clip_rect(200.0, 0.0, 300.0, 10.0, (100, 100)), None);
        assert_eq!(clip_rect(10.0, 10.0, 10.0, 50.0, (100, 100)), None);
    }

    #[test]
    fn globals_layout_matches_wgsl_std140() {
        assert_eq!(std::mem::size_of::<Globals>(), 48);
        assert_eq!(std::mem::size_of::<Instance>(), 32);
    }

    #[test]
    fn grid_is_omitted_when_too_dense_and_capped() {
        let mut doc = Document::new(DocId(1), "t", Size::new(60_000, 1000)).unwrap();
        doc.guides.clear();
        let frame = FrameParams {
            document: Some(&doc),
            viewport: Viewport { zoom: 0.02, origin: Vec2::default() },
            settings: ViewSettings { show_grid: true, show_guides: false, grid_spacing: 1, ..ViewSettings::default() },
            theme: Theme::Light,
            preview_guide: None,
        };
        // 1 px de doc à 2 % = 0,02 px d'écran : seul le contour (4 segments) doit rester.
        assert_eq!(build_lines(&frame, (800, 600)).len(), 8);
    }

    /// Rendu réel hors écran ; ignoré (avec message) si la machine n'a aucun adaptateur.
    fn headless() -> Option<Renderer> {
        match Renderer::new_headless() {
            Ok(r) => Some(r),
            // En CI on veut un échec franc plutôt qu'un test « vert » qui n'a rien rendu.
            Err(e) if std::env::var_os("OPENPICS_REQUIRE_GPU").is_some() => {
                panic!("GPU requis : {e}")
            }
            Err(e) => {
                eprintln!("test GPU ignoré : {e}");
                None
            }
        }
    }

    fn pixel(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    #[test]
    fn renders_tiles_at_the_right_place_and_zoom() {
        let Some(mut r) = headless() else { return };
        let mut doc = Document::new(DocId(1), "t", Size::new(300, 300)).unwrap();
        // Un pixel rouge dans la deuxième tuile en X, un dans la première (tuiles « Pixels », donc texturées).
        doc.layers[0].pixels.put_pixel(10, 10, [255, 0, 0, 255]);
        doc.layers[0].pixels.put_pixel(280, 5, [0, 0, 255, 255]);
        let frame = FrameParams {
            document: Some(&doc),
            viewport: Viewport { zoom: 2.0, origin: Vec2::new(20.0, 30.0) },
            settings: ViewSettings { show_grid: false, show_guides: false, ..ViewSettings::default() },
            theme: Theme::Light,
            preview_guide: None,
        };
        let img = r.render_to_rgba8(&frame, (700, 700)).unwrap();
        // pixel doc (10,10) -> écran (20+20 .. 20+22, 30+20 .. 30+22)
        assert_eq!(pixel(&img, 700, 41, 51), [255, 0, 0, 255]);
        assert_eq!(pixel(&img, 700, 20 + 560 + 1, 30 + 10 + 1), [0, 0, 255, 255]);
        // Hors document : fond du canvas, pas du damier.
        let bg = Theme::Light.canvas_background();
        let outside = pixel(&img, 700, 5, 5);
        assert_eq!(outside[0], (bg.r * 255.0).round() as u8);
        // Dans le document : damier blanc/gris (aucune tuile à cet endroit).
        let inside = pixel(&img, 700, 25, 35);
        assert!(inside[0] >= 200, "{inside:?}");
    }

    #[test]
    fn modified_tile_is_reuploaded() {
        let Some(mut r) = headless() else { return };
        let mut doc = Document::new(DocId(1), "t", Size::new(64, 64)).unwrap();
        doc.layers[0].pixels.put_pixel(1, 1, [255, 0, 0, 255]);
        fn frame(d: &Document) -> FrameParams<'_> {
            FrameParams {
                document: Some(d),
                viewport: Viewport { zoom: 4.0, origin: Vec2::default() },
                settings: ViewSettings { show_grid: false, show_guides: false, ..ViewSettings::default() },
                theme: Theme::Dark,
                preview_guide: None,
            }
        }
        let a = r.render_to_rgba8(&frame(&doc), (300, 300)).unwrap();
        assert_eq!(pixel(&a, 300, 6, 6), [255, 0, 0, 255]);
        doc.layers[0].pixels.put_pixel(1, 1, [0, 255, 0, 255]);
        let b = r.render_to_rgba8(&frame(&doc), (300, 300)).unwrap();
        assert_eq!(pixel(&b, 300, 6, 6), [0, 255, 0, 255]);
    }

    #[test]
    fn layers_with_blend_modes_are_composited_on_screen() {
        let Some(mut r) = headless() else { return };
        let mut doc = Document::new(DocId(1), "t", Size::new(64, 64)).unwrap();
        doc.layers[0].pixels.put_pixel(2, 2, [51, 128, 204, 255]);
        let mut top = openpics_core::TileGrid::new(64, 64).unwrap();
        top.put_pixel(2, 2, [153, 77, 230, 255]);
        let id = doc.add_layer("haut", top);
        doc.layer_mut(id).unwrap().blend = openpics_core::BlendMode::Multiply;
        let frame = FrameParams {
            document: Some(&doc),
            viewport: Viewport { zoom: 4.0, origin: Vec2::default() },
            settings: ViewSettings { show_grid: false, show_guides: false, ..ViewSettings::default() },
            theme: Theme::Light,
            preview_guide: None,
        };
        let a = r.render_to_rgba8(&frame, (300, 300)).unwrap();
        assert_eq!(pixel(&a, 300, 10, 10), [31, 39, 184, 255], "produit attendu, pas le dernier calque tel quel");
        // Une modification suivante doit être revue (cache invalidé par la révision).
        let mut doc2 = doc.render_snapshot();
        doc2.layer_mut(id).unwrap().blend = openpics_core::BlendMode::Normal;
        doc2.touch();
        let frame2 = FrameParams { document: Some(&doc2), ..frame };
        let b = r.render_to_rgba8(&frame2, (300, 300)).unwrap();
        assert_eq!(pixel(&b, 300, 10, 10), [153, 77, 230, 255]);
    }

    #[test]
    fn guides_are_drawn_over_the_canvas() {
        let Some(mut r) = headless() else { return };
        let mut doc = Document::new(DocId(1), "t", Size::new(100, 100)).unwrap();
        doc.execute(Box::new(openpics_core::commands::AddGuide::new(openpics_core::Axis::Vertical, 50.0))).unwrap();
        let frame = FrameParams {
            document: Some(&doc),
            viewport: Viewport { zoom: 1.0, origin: Vec2::new(10.0, 10.0) },
            settings: ViewSettings::default(),
            theme: Theme::Light,
            preview_guide: None,
        };
        let img = r.render_to_rgba8(&frame, (200, 200)).unwrap();
        let p = pixel(&img, 200, 60, 150);
        assert!(p[2] > p[0] && p[1] > 150, "repère cyan attendu, obtenu {p:?}");
    }
}
