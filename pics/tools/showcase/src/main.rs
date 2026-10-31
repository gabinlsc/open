//! Génère les scènes de démonstration avec le vrai moteur : pour chacune, l'état envoyé à l'interface
//! (`<nom>.json`) et le rendu du canvas par le vrai moteur de rendu (`<nom>.png`).
//!
//! Usage : `showcase <dossier de sortie> <largeur canvas> <hauteur canvas>` (taille mesurée dans l'interface).
//! Sans GPU, le moteur logiciel (lavapipe) est utilisé.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // outil de développement, pas de l'application

use std::path::{Path, PathBuf};

use openpics_app::dto::{ResolvedTheme, ShapeDto, TextAlignDto, TextDto, Tool};
use openpics_app::session::{Pointer, Session};
use openpics_core::shortcuts::Keymap;
use openpics_core::{Vec2, ViewSettings};
use openpics_gpu::{FrameParams, Renderer, Theme};

struct Ctx {
    out: PathBuf,
    canvas: (u32, u32),
    renderer: Renderer,
}

fn session(ctx: &Ctx, theme: ResolvedTheme) -> Session {
    let mut s = Session::new(ViewSettings::default(), Keymap::default());
    s.theme = theme;
    s.set_canvas(ctx.canvas.0, ctx.canvas.1);
    s
}

fn orange(s: &mut Session) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/orange.png");
    s.open_path(&path).expect("ouvrir l'illustration");
    s.fit().unwrap();
}

/// Écrit `<nom>.json` (état pour l'interface) et `<nom>.png` (canvas rendu).
fn dump(ctx: &mut Ctx, name: &str, s: &Session) {
    let snap = serde_json::to_value(s.snapshot()).unwrap();
    let theme = if s.theme == ResolvedTheme::Dark { "dark" } else { "light" };
    let scene = serde_json::json!({
        "snapshot": snap,
        "keymap": s.keymap_dto(),
        "status": { "version": "0.1.0", "portable": false, "gpuError": null, "theme": theme },
    });
    std::fs::write(ctx.out.join(format!("{name}.json")), serde_json::to_vec(&scene).unwrap()).unwrap();
    let state = s.render_state();
    let frame = FrameParams {
        document: state.document.as_ref(),
        viewport: state.viewport,
        settings: state.settings,
        theme: if state.theme == ResolvedTheme::Dark { Theme::Dark } else { Theme::Light },
        preview_guide: None,
        selection: state.selection.as_ref(),
    };
    let rgba = ctx.renderer.render_to_rgba8(&frame, ctx.canvas).expect("rendu");
    image::save_buffer(ctx.out.join(format!("{name}.png")), &rgba, ctx.canvas.0, ctx.canvas.1, image::ColorType::Rgba8)
        .unwrap();
    println!("scène {name}");
}

/// Rendu seul du document (sans interface), à la taille du canvas de la session : sert à la vidéo.
fn art(ctx: &mut Ctx, name: &str, s: &Session) {
    let state = s.render_state();
    let size = s.canvas_size();
    let frame = FrameParams {
        document: state.document.as_ref(),
        viewport: state.viewport,
        settings: state.settings,
        theme: Theme::Light,
        preview_guide: None,
        selection: state.selection.as_ref(),
    };
    let rgba = ctx.renderer.render_to_rgba8(&frame, (size.width, size.height)).expect("rendu");
    let dir = ctx.out.join("art");
    std::fs::create_dir_all(&dir).unwrap();
    image::save_buffer(dir.join(format!("{name}.png")), &rgba, size.width, size.height, image::ColorType::Rgba8)
        .unwrap();
    println!("art {name}");
}

/// Session dont le canvas a exactement la taille du document, affiché à 100 % : le rendu est le document seul.
fn art_session(ctx: &Ctx, w: u32, h: u32) -> Session {
    let mut s = Session::new(
        ViewSettings { show_rulers: false, show_grid: false, show_guides: false, ..ViewSettings::default() },
        Keymap::default(),
    );
    let _ = ctx;
    s.set_canvas(w, h);
    s
}

fn drag(s: &mut Session, a: (f64, f64), b: (f64, f64)) {
    let p = |(x, y): (f64, f64)| {
        let v = s.viewport().doc_to_screen(Vec2::new(x, y));
        Pointer { x: v.x, y: v.y, pressure: 1.0, ..Default::default() }
    };
    let (pa, pb) = (p(a), p(b));
    s.pointer_down(&pa).unwrap();
    s.pointer_move(&Pointer { x: (pa.x + pb.x) / 2.0, y: (pa.y + pb.y) / 2.0, ..pb }).unwrap();
    s.pointer_move(&pb).unwrap();
    s.pointer_up(&pb).unwrap();
}

fn shape_style(s: &mut Session, fill: Option<(&str, f32)>, stroke: Option<(&str, f32)>) {
    let id = s.snapshot().active_layer_id.unwrap();
    let dto = ShapeDto {
        fill: fill.map(|(c, o)| openpics_app::dto::ColorOpacityDto { color: c.into(), opacity: o }),
        stroke: stroke.map(|(c, w)| openpics_app::dto::StrokeDto {
            color: c.into(),
            opacity: 1.0,
            width: w,
            round_caps: true,
        }),
        even_odd: false,
        path_count: 0,
    };
    s.shape_set_style(id, &dto).unwrap();
}

fn text(s: &mut Session, at: (f64, f64), content: &str, size: f32, color: &str, bold: bool) {
    let id = s.text_add(at.0, at.1).unwrap();
    let dto = TextDto {
        text: content.into(),
        font: String::new(),
        size,
        color: color.into(),
        bold,
        italic: false,
        align: TextAlignDto::Left,
        line_height: 1.2,
        box_width: None,
        origin_x: at.0 as f32,
        origin_y: at.1 as f32,
        has_path: false,
    };
    s.text_set(id, &dto).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, out, w, h] = args.as_slice() else { panic!("usage : showcase <sortie> <largeur> <hauteur>") };
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).unwrap();
    let canvas = (w.parse().unwrap(), h.parse().unwrap());
    let renderer = Renderer::new_headless().expect("rendu logiciel ou GPU");
    let mut ctx = Ctx { out, canvas, renderer };

    // 01 — l'éditeur : retouche non destructive par calques de réglage.
    for (name, theme) in [("01-editor", ResolvedTheme::Light), ("01-editor-dark", ResolvedTheme::Dark)] {
        let mut s = session(&ctx, theme);
        orange(&mut s);
        let v = s.adjustment_add("vibrance").unwrap();
        s.adjustment_set_param(v, "vibrance", 0.28).unwrap();
        let hs = s.adjustment_add("hue_saturation").unwrap();
        s.adjustment_set_param(hs, "saturation", 0.05).unwrap();
        let bc = s.adjustment_add("brightness_contrast").unwrap();
        s.adjustment_set_param(bc, "contrast", 0.1).unwrap();
        s.layer_select(hs).unwrap();
        dump(&mut ctx, name, &s);
    }

    // 02 — sélection du sujet, puis 03 — suppression de l'arrière-plan (masque réversible).
    let mut s = session(&ctx, ResolvedTheme::Light);
    orange(&mut s);
    s.ai_select_subject(0.12).unwrap();
    dump(&mut ctx, "02-subject", &s);
    s.deselect().unwrap();
    s.ai_remove_background(0.12).unwrap();
    dump(&mut ctx, "03-background", &s);

    // 04 — vectoriel et texte : une affiche entièrement faite dans OpenPics.
    let mut s = session(&ctx, ResolvedTheme::Light);
    s.new_document(Some("Affiche".into()), 1600, 1000).unwrap();
    s.fit().unwrap();
    s.set_tool(Tool::ShapeRect);
    drag(&mut s, (0.0, 0.0), (1600.0, 1000.0));
    shape_style(&mut s, Some(("#2447d6", 1.0)), None);
    s.set_tool(Tool::ShapeEllipse);
    drag(&mut s, (980.0, 120.0), (1480.0, 620.0));
    shape_style(&mut s, Some(("#ffffff", 0.14)), None);
    drag(&mut s, (1100.0, 560.0), (1380.0, 840.0));
    shape_style(&mut s, None, Some(("#ffffff", 8.0)));
    s.set_tool(Tool::ShapeRoundedRect);
    drag(&mut s, (120.0, 700.0), (560.0, 780.0));
    shape_style(&mut s, Some(("#ffffff", 1.0)), None);
    text(&mut s, (120.0, 250.0), "OpenPics", 190.0, "#ffffff", true);
    text(&mut s, (126.0, 470.0), "L'éditeur d'images libre, local et sans compromis.", 46.0, "#dbe4ff", false);
    text(&mut s, (156.0, 716.0), "Gratuit · GPLv3", 40.0, "#2447d6", true);
    dump(&mut ctx, "04-vector", &s);

    // 05 — filtres : aperçu en direct (l'état d'interface « filtre ouvert » est posé par la capture).
    let mut s = session(&ctx, ResolvedTheme::Light);
    orange(&mut s);
    s.filter_preview("crystallize", &[22.0, 3.0]).unwrap();
    dump(&mut ctx, "05-filters", &s);
    s.filter_cancel();
    // --- Rendus « art » pour la vidéo (document seul, 100 %) ---
    let mut a = art_session(&ctx, 1600, 1067);
    orange(&mut a);
    a.actual_size().unwrap();
    art(&mut ctx, "orange-original", &a);
    let v = a.adjustment_add("vibrance").unwrap();
    a.adjustment_set_param(v, "vibrance", 0.28).unwrap();
    let hs = a.adjustment_add("hue_saturation").unwrap();
    a.adjustment_set_param(hs, "saturation", 0.05).unwrap();
    let bc = a.adjustment_add("brightness_contrast").unwrap();
    a.adjustment_set_param(bc, "contrast", 0.1).unwrap();
    art(&mut ctx, "orange-retouche", &a);

    let mut a = art_session(&ctx, 1600, 1067);
    orange(&mut a);
    a.actual_size().unwrap();
    a.ai_select_subject(0.12).unwrap();
    art(&mut ctx, "orange-selection", &a);
    a.deselect().unwrap();
    a.ai_remove_background(0.12).unwrap();
    art(&mut ctx, "orange-sans-fond", &a);

    for (i, cell) in [3.0f32, 6.0, 10.0, 16.0, 24.0, 36.0].into_iter().enumerate() {
        let mut a = art_session(&ctx, 1600, 1067);
        orange(&mut a);
        a.actual_size().unwrap();
        a.filter_preview("crystallize", &[cell, 3.0]).unwrap();
        art(&mut ctx, &format!("cristal-{i}"), &a);
    }

    // L'affiche, construite pas à pas : chaque étape est un vrai rendu du moteur.
    let mut a = art_session(&ctx, 1600, 1000);
    a.new_document(Some("Affiche".into()), 1600, 1000).unwrap();
    a.actual_size().unwrap();
    a.set_tool(Tool::ShapeRect);
    drag(&mut a, (0.0, 0.0), (1600.0, 1000.0));
    shape_style(&mut a, Some(("#2447d6", 1.0)), None);
    art(&mut ctx, "affiche-0", &a);
    a.set_tool(Tool::ShapeEllipse);
    drag(&mut a, (980.0, 120.0), (1480.0, 620.0));
    shape_style(&mut a, Some(("#ffffff", 0.14)), None);
    drag(&mut a, (1100.0, 560.0), (1380.0, 840.0));
    shape_style(&mut a, None, Some(("#ffffff", 8.0)));
    art(&mut ctx, "affiche-1", &a);
    a.set_tool(Tool::ShapeRoundedRect);
    drag(&mut a, (120.0, 700.0), (560.0, 780.0));
    shape_style(&mut a, Some(("#ffffff", 1.0)), None);
    text(&mut a, (156.0, 716.0), "Gratuit · GPLv3", 40.0, "#2447d6", true);
    art(&mut ctx, "affiche-2", &a);
    text(&mut a, (120.0, 250.0), "OpenPics", 190.0, "#ffffff", true);
    art(&mut ctx, "affiche-3", &a);
    text(&mut a, (126.0, 470.0), "L'éditeur d'images libre, local et sans compromis.", 46.0, "#dbe4ff", false);
    art(&mut ctx, "affiche-4", &a);

    println!("terminé : {}", ctx.out.display());
}
