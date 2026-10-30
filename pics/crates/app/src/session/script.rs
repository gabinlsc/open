//! Scripts et actions enregistrées.
//!
//! Un script Rhai ne touche jamais au document directement : il **décrit** une suite d'étapes (`filter(...)`,
//! `resize(...)`…) qui sont exécutées ensuite, une à une, par la session. Le moteur n'a ni module, ni accès aux
//! fichiers, ni réseau ; le nombre d'opérations, la profondeur et la taille des données sont plafonnés, donc
//! une boucle infinie échoue au lieu de figer l'application.
//!
//! L'enregistrement d'actions produit du même texte : ce qu'on enregistre peut être relu, édité et rejoué.

use std::sync::{Arc, Mutex};

use rhai::{Array, Dynamic, Engine, Scope};

use super::{Session, SessionError};
use crate::dto::ImageOp;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Step {
    Filter(String, Vec<f32>),
    Op(ImageOp),
    SelectAll,
    Deselect,
    InvertSelection,
    Subject(f32),
    RemoveBackground(f32),
    Inpaint,
    Resize(u32, u32),
    NewLayer,
}

const MAX_SOURCE: usize = 20_000;
const MAX_STEPS: usize = 500;

fn floats(values: &Array) -> Vec<f32> {
    values
        .iter()
        .filter_map(|v| v.as_float().ok().map(|f| f as f32).or_else(|| v.as_int().ok().map(|i| i as f32)))
        .collect()
}

fn number(v: &Dynamic) -> Result<f32, Box<rhai::EvalAltResult>> {
    v.as_float().map(|f| f as f32).or_else(|_| v.as_int().map(|i| i as f32)).map_err(|_| "un nombre est attendu".into())
}

fn to_u32(v: i64) -> Result<u32, Box<rhai::EvalAltResult>> {
    u32::try_from(v).map_err(|_| format!("dimension hors limites : {v}").into())
}

/// Transforme un script en étapes. `size` = taille du document actif, exposée comme `width` et `height`.
pub(super) fn parse(src: &str, size: (u32, u32)) -> Result<Vec<Step>, String> {
    if src.len() > MAX_SOURCE {
        return Err(format!("script trop long ({} caractères maximum)", MAX_SOURCE));
    }
    let steps: Arc<Mutex<Vec<Step>>> = Arc::new(Mutex::new(Vec::new()));
    let mut engine = Engine::new();
    engine.set_max_operations(200_000);
    engine.set_max_call_levels(32);
    engine.set_max_expr_depths(64, 32);
    engine.set_max_string_size(1024);
    engine.set_max_array_size(64);
    engine.set_max_map_size(16);
    engine.disable_symbol("eval");

    let push = |steps: &Arc<Mutex<Vec<Step>>>, step: Step| -> Result<(), Box<rhai::EvalAltResult>> {
        let mut guard = steps.lock().map_err(|_| "état du script corrompu")?;
        if guard.len() >= MAX_STEPS {
            return Err(format!("trop d'étapes ({MAX_STEPS} maximum)").into());
        }
        guard.push(step);
        Ok(())
    };
    {
        let s = steps.clone();
        engine.register_fn("filter", move |id: &str, values: Array| {
            push(&s, Step::Filter(id.to_owned(), floats(&values)))
        });
    }
    {
        let s = steps.clone();
        engine.register_fn("filter", move |id: &str| push(&s, Step::Filter(id.to_owned(), Vec::new())));
    }
    {
        let s = steps.clone();
        engine.register_fn("op", move |name: &str| {
            let op = match name {
                "flip_h" => ImageOp::FlipHorizontal,
                "flip_v" => ImageOp::FlipVertical,
                "invert" => ImageOp::Invert,
                other => return Err(format!("opération inconnue : {other} (flip_h, flip_v, invert)").into()),
            };
            push(&s, Step::Op(op))
        });
    }
    for (name, step) in [
        ("select_all", Step::SelectAll),
        ("deselect", Step::Deselect),
        ("invert_selection", Step::InvertSelection),
        ("inpaint", Step::Inpaint),
        ("new_layer", Step::NewLayer),
    ] {
        let s = steps.clone();
        engine.register_fn(name, move || push(&s, step.clone()));
    }
    {
        let s = steps.clone();
        engine.register_fn("select_subject", move |tol: Dynamic| push(&s, Step::Subject(number(&tol)?)));
    }
    {
        let s = steps.clone();
        engine.register_fn("remove_background", move |tol: Dynamic| push(&s, Step::RemoveBackground(number(&tol)?)));
    }
    {
        let s = steps.clone();
        engine.register_fn("resize", move |w: i64, h: i64| push(&s, Step::Resize(to_u32(w)?, to_u32(h)?)));
    }
    let mut scope = Scope::new();
    scope.push_constant("width", Dynamic::from(i64::from(size.0)));
    scope.push_constant("height", Dynamic::from(i64::from(size.1)));
    engine.run_with_scope(&mut scope, src).map_err(|e| e.to_string())?;
    let out = steps.lock().map_err(|_| "état du script corrompu".to_string())?.clone();
    Ok(out)
}

/// Texte de script correspondant à une étape (utilisé par l'enregistrement d'actions).
pub(super) fn line(step: &Step) -> String {
    match step {
        Step::Filter(id, v) => {
            let list: Vec<String> = v.iter().map(|x| format!("{x:?}")).collect();
            format!("filter(\"{id}\", [{}]);", list.join(", "))
        }
        Step::Op(ImageOp::FlipHorizontal) => "op(\"flip_h\");".into(),
        Step::Op(ImageOp::FlipVertical) => "op(\"flip_v\");".into(),
        Step::Op(ImageOp::Invert) => "op(\"invert\");".into(),
        Step::SelectAll => "select_all();".into(),
        Step::Deselect => "deselect();".into(),
        Step::InvertSelection => "invert_selection();".into(),
        Step::Subject(t) => format!("select_subject({t:?});"),
        Step::RemoveBackground(t) => format!("remove_background({t:?});"),
        Step::Inpaint => "inpaint();".into(),
        Step::Resize(w, h) => format!("resize({w}, {h});"),
        Step::NewLayer => "new_layer();".into(),
    }
}

impl Session {
    pub(super) fn record(&mut self, step: Step) {
        if let Some(lines) = &mut self.recording {
            lines.push(line(&step));
        }
    }

    pub fn record_start(&mut self) {
        self.recording = Some(Vec::new());
    }

    /// Termine l'enregistrement et renvoie le script obtenu.
    pub fn record_stop(&mut self) -> String {
        let lines = self.recording.take().unwrap_or_default();
        lines.join("\n")
    }

    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Exécute un script. Renvoie le nombre d'étapes réalisées ; une étape qui échoue arrête le script, et ce qui
    /// précède reste dans l'historique (annulable étape par étape).
    pub fn script_run(&mut self, src: &str) -> Result<usize, SessionError> {
        let size = self.entry()?.doc.size();
        let steps = parse(src, (size.width, size.height))
            .map_err(|e| SessionError::InvalidArgument(format!("script : {e}")))?;
        // Rejouer un script pendant un enregistrement ne doit pas l'enregistrer deux fois.
        let recording = self.recording.take();
        let result = self.run_steps(&steps);
        self.recording = recording;
        result.map(|()| steps.len())
    }

    fn run_steps(&mut self, steps: &[Step]) -> Result<(), SessionError> {
        for (i, step) in steps.iter().enumerate() {
            let result = match step {
                Step::Filter(id, values) => match openpics_ops::filters::find(id) {
                    None => Err(SessionError::InvalidArgument(format!("filtre inconnu : {id}"))),
                    Some(def) => {
                        let values = if values.is_empty() { def.defaults() } else { values.clone() };
                        self.filter_preview(id, &values).and_then(|()| self.filter_commit())
                    }
                },
                Step::Op(op) => self.apply_op(*op).map(|_| ()),
                Step::SelectAll => self.select_all(),
                Step::Deselect => self.deselect(),
                Step::InvertSelection => self.select_invert(),
                Step::Subject(t) => self.ai_select_subject(*t),
                Step::RemoveBackground(t) => self.ai_remove_background(*t),
                Step::Inpaint => self.ai_inpaint(),
                Step::Resize(w, h) => self.image_resize(*w, *h),
                Step::NewLayer => self.layer_add().map(|_| ()),
            };
            if let Err(e) = result {
                self.filter_cancel();
                return Err(SessionError::InvalidArgument(format!("étape {} ({}) : {e}", i + 1, line(step))));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::ViewSettings;
    use openpics_core::shortcuts::Keymap;

    #[test]
    fn scripts_describe_steps_with_loops_and_document_size() {
        let steps = parse(
            "for i in 0..3 { filter(\"solarize\", [0.5]); }\nresize(width * 2, height);\nop(\"invert\");",
            (10, 4),
        )
        .unwrap();
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0], Step::Filter("solarize".into(), vec![0.5]));
        assert_eq!(steps[3], Step::Resize(20, 4));
        assert_eq!(steps[4], Step::Op(ImageOp::Invert));
        assert_eq!(parse("filter(\"x\");", (1, 1)).unwrap(), vec![Step::Filter("x".into(), vec![])]);
        assert_eq!(parse("select_subject(0.2); remove_background(1); inpaint();", (1, 1)).unwrap().len(), 3);
    }

    #[test]
    fn the_sandbox_stops_runaway_scripts_and_offers_no_escape_hatch() {
        assert!(parse("loop { }", (1, 1)).unwrap_err().to_lowercase().contains("operations"));
        assert!(parse("while true { filter(\"a\"); }", (1, 1)).is_err(), "plafond d'étapes ou d'opérations");
        assert!(parse("eval(\"1+1\");", (1, 1)).is_err());
        assert!(parse("import \"x\" as m;", (1, 1)).is_err(), "pas de modules");
        assert!(parse("open_file(\"/etc/passwd\");", (1, 1)).is_err(), "aucune fonction de fichier");
        assert!(parse("op(\"rm -rf\");", (1, 1)).is_err());
        assert!(parse("resize(-5, 5);", (1, 1)).is_err());
        assert!(parse(&"a".repeat(MAX_SOURCE + 1), (1, 1)).is_err());
        assert!(parse("this is not rhai", (1, 1)).is_err());
    }

    fn session() -> Session {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.set_canvas(800, 600);
        s.new_document(None, 10, 10).unwrap();
        s
    }

    #[test]
    fn a_script_runs_step_by_step_and_a_failure_names_the_step() {
        let mut s = session();
        assert_eq!(s.script_run("filter(\"solarize\", [0.5]);\nresize(20, 20);").unwrap(), 2);
        let snap = s.snapshot();
        assert_eq!(snap.docs[0].width, 20);
        assert_eq!(s.entry().unwrap().doc.layers[0].pixels.get_pixel(3, 3), [0, 0, 0, 255]);
        let err = s.script_run("op(\"invert\");\nfilter(\"nope\");").unwrap_err().to_string();
        assert!(err.contains("étape 2") && err.contains("nope"), "{err}");
    }

    #[test]
    fn recording_produces_a_script_that_replays_the_same_result() {
        let mut s = session();
        s.record_start();
        s.filter_preview("solarize", &[0.5]).unwrap();
        s.filter_commit().unwrap();
        s.apply_op(ImageOp::FlipHorizontal).unwrap();
        s.image_resize(20, 10).unwrap();
        s.select_all().unwrap();
        let script = s.record_stop();
        assert!(!s.is_recording());
        assert_eq!(script, "filter(\"solarize\", [0.5]);\nop(\"flip_h\");\nresize(20, 10);\nselect_all();");
        let mut t = session();
        t.script_run(&script).unwrap();
        assert_eq!(t.snapshot().docs[0].width, 20);
        assert!(t.snapshot().has_selection);
        assert_eq!(
            t.entry().unwrap().doc.layers[0].pixels.get_pixel(3, 3),
            s.entry().unwrap().doc.layers[0].pixels.get_pixel(3, 3)
        );
        // rejouer pendant un enregistrement n'enregistre pas deux fois
        t.record_start();
        t.script_run("op(\"invert\");").unwrap();
        assert_eq!(t.record_stop(), "");
    }
}
