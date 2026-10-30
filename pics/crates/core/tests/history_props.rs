//! Propriété : quelle que soit la séquence d'actions sur les calques, « tout annuler » restitue exactement
//! chaque état précédent, et « tout rétablir » rejoue exactement chaque état suivant.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use openpics_core::commands::{
    AddGuide, AddLayer, AddMask, MoveLayer, RemoveLayer, RemoveMask, ReplaceTiles, SetLayerProps, SetLayerStyles,
    SetMaskEnabled,
};
use openpics_core::styles::StrokePosition;
use openpics_core::{Axis, BlendMode, DocId, Document, Layer, LayerId, LayerStyle, Size, TileGrid};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Step {
    AddLayer(usize),
    RemoveLayer(usize),
    MoveLayer(usize, usize),
    Opacity(usize, f32),
    Blend(usize, usize),
    Clip(usize),
    Mask(usize, bool),
    RemoveMask(usize),
    ToggleMask(usize),
    Style(usize, u8),
    Paint(usize, u32, u32, [u8; 4]),
    Guide(bool, f64),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0usize..6).prop_map(Step::AddLayer),
        (0usize..6).prop_map(Step::RemoveLayer),
        (0usize..6, 0usize..6).prop_map(|(a, b)| Step::MoveLayer(a, b)),
        (0usize..6, 0.0f32..=1.0).prop_map(|(a, o)| Step::Opacity(a, o)),
        (0usize..6, 0usize..27).prop_map(|(a, m)| Step::Blend(a, m)),
        (0usize..6).prop_map(Step::Clip),
        (0usize..6, any::<bool>()).prop_map(|(a, r)| Step::Mask(a, r)),
        (0usize..6).prop_map(Step::RemoveMask),
        (0usize..6).prop_map(Step::ToggleMask),
        (0usize..6, 0u8..4).prop_map(|(a, k)| Step::Style(a, k)),
        (0usize..6, 0u32..40, 0u32..40, any::<[u8; 4]>()).prop_map(|(a, x, y, c)| Step::Paint(a, x, y, c)),
        (any::<bool>(), 0.0f64..40.0).prop_map(|(v, p)| Step::Guide(v, p)),
    ]
}

/// Empreinte FNV-1a 64 bits d'un tampon : assez discriminante pour comparer des images, et compacte
/// pour que l'échec d'un test reste lisible.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

/// Empreinte complète et déterministe de l'état éditable du document.
fn fingerprint(d: &Document) -> String {
    let mut out = String::new();
    for l in &d.layers {
        let mask = l.mask.as_ref().map(|m| (m.enabled, fnv(&m.pixels.to_rgba8())));
        out += &format!(
            "{:?}|{}|{}|{}|{}|{:?}|{:?}|{}|{:?}|{:?}|{:?}\n",
            l.id,
            l.name,
            l.visible,
            l.opacity,
            l.fill,
            l.blend,
            l.locks,
            l.clip_to_below,
            l.styles,
            mask,
            l.pixels.to_rgba8()
        );
    }
    out + &format!("{:?}", d.guides)
}

fn pick(d: &Document, i: usize) -> Option<LayerId> {
    (!d.layers.is_empty()).then(|| d.layers[i % d.layers.len()].id)
}

fn run(d: &mut Document, s: &Step) -> bool {
    let size = d.size();
    let result = match s {
        Step::AddLayer(i) => {
            let id = LayerId(d.alloc_id());
            let layer = Layer::new(id, format!("Calque {}", id.0), TileGrid::new(size.width, size.height).unwrap());
            d.execute(Box::new(AddLayer::new(i % (d.layers.len() + 1), layer)))
        }
        Step::RemoveLayer(i) => match pick(d, *i) {
            Some(id) => d.execute(Box::new(RemoveLayer::new(id))),
            None => return false,
        },
        Step::MoveLayer(i, to) => match pick(d, *i) {
            Some(id) => d.execute(Box::new(MoveLayer::new(id, to % d.layers.len()))),
            None => return false,
        },
        Step::Opacity(i, o) => match pick(d, *i) {
            Some(id) => {
                let mut p = d.layer(id).unwrap().props();
                p.opacity = *o;
                d.execute(Box::new(SetLayerProps::new("opacité", id, p)))
            }
            None => return false,
        },
        Step::Blend(i, m) => match pick(d, *i) {
            Some(id) => {
                let mut p = d.layer(id).unwrap().props();
                p.blend = BlendMode::ALL[*m];
                d.execute(Box::new(SetLayerProps::new("fusion", id, p)))
            }
            None => return false,
        },
        Step::Clip(i) => match pick(d, *i) {
            Some(id) => {
                let mut p = d.layer(id).unwrap().props();
                p.clip_to_below = !p.clip_to_below;
                d.execute(Box::new(SetLayerProps::new("écrêtage", id, p)))
            }
            None => return false,
        },
        Step::Mask(i, reveal) => match pick(d, *i) {
            Some(id) => d.execute(Box::new(AddMask::new(id, *reveal))),
            None => return false,
        },
        Step::RemoveMask(i) => match pick(d, *i) {
            Some(id) => d.execute(Box::new(RemoveMask::new(id))),
            None => return false,
        },
        Step::ToggleMask(i) => match pick(d, *i) {
            Some(id) => {
                let on = d.layer(id).unwrap().mask.as_ref().is_some_and(|m| m.enabled);
                d.execute(Box::new(SetMaskEnabled::new(id, !on)))
            }
            None => return false,
        },
        Step::Style(i, k) => match pick(d, *i) {
            Some(id) => {
                let style = match k {
                    0 => LayerStyle::DropShadow {
                        color: [0; 3],
                        opacity: 0.5,
                        angle_deg: 30.0,
                        distance: 3.0,
                        size: 2.0,
                    },
                    1 => LayerStyle::OuterGlow { color: [255, 255, 0], opacity: 1.0, size: 3.0 },
                    2 => LayerStyle::Stroke {
                        color: [1, 2, 3],
                        opacity: 1.0,
                        size: 2.0,
                        position: StrokePosition::Center,
                    },
                    _ => LayerStyle::ColorOverlay { color: [9; 3], opacity: 0.4, blend: BlendMode::Multiply },
                };
                d.execute(Box::new(SetLayerStyles::new("style", id, vec![style])))
            }
            None => return false,
        },
        Step::Paint(i, x, y, c) => match pick(d, *i) {
            Some(id) => {
                let old = d.layer(id).unwrap().pixels.clone();
                let mut new = old.clone();
                new.put_pixel(*x, *y, *c);
                match ReplaceTiles::between("peindre", id, &old, &new) {
                    Some(cmd) => d.execute(Box::new(cmd)),
                    None => return false,
                }
            }
            None => return false,
        },
        Step::Guide(vertical, p) => {
            d.execute(Box::new(AddGuide::new(if *vertical { Axis::Vertical } else { Axis::Horizontal }, *p)))
        }
    };
    result.is_ok()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn undo_all_then_redo_all_restores_every_state(steps in proptest::collection::vec(step(), 1..25)) {
        let mut d = Document::new(DocId(1), "p", Size::new(40, 40)).unwrap();
        let mut states = vec![fingerprint(&d)];
        for s in &steps {
            // Une action refusée (index invalide, masque déjà présent…) ne doit rien changer ni entrer dans l'historique.
            let before = fingerprint(&d);
            if run(&mut d, s) {
                states.push(fingerprint(&d));
            } else {
                prop_assert_eq!(fingerprint(&d), before, "une action refusée a modifié le document : {:?}", s);
            }
        }
        // Tout annuler : on retrouve chaque état précédent, dans l'ordre inverse.
        for expected in states.iter().rev().skip(1) {
            prop_assert!(d.undo().unwrap());
            prop_assert_eq!(&fingerprint(&d), expected);
        }
        prop_assert!(!d.undo().unwrap());
        // Tout rétablir : on rejoue chaque état.
        for expected in states.iter().skip(1) {
            prop_assert!(d.redo().unwrap());
            prop_assert_eq!(&fingerprint(&d), expected);
        }
        prop_assert!(!d.redo().unwrap());
    }
}
