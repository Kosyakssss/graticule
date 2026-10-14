use std::path::PathBuf;

use ledger::Value;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::Window;

pub(crate) const ENV: &str = "GRATICULE_RELOAD";

pub(crate) struct Reload {
    handoff: PathBuf,
}

pub(crate) struct Handoff {
    pub frame: Option<(PhysicalPosition<i32>, PhysicalSize<u32>)>,
    pub face: Value,
}

impl Reload {
    pub fn from_env() -> Option<Reload> {
        Some(Reload { handoff: PathBuf::from(std::env::var_os(ENV)?) })
    }

    pub fn listen(&self, reload: impl Fn() + Send + 'static) {
        let _ = std::thread::Builder::new().name("reload".into()).spawn(move || {
            for _ in std::io::stdin().lines().map_while(Result::ok) {
                reload();
            }
        });
    }

    pub fn take(&self) -> Option<Handoff> {
        let bytes = std::fs::read(&self.handoff).ok()?;
        let _ = std::fs::remove_file(&self.handoff);
        let value = Value::from_bytes(&bytes).ok()?;
        let frame = match value.field("frame") {
            Some(Value::List(n)) => match n.as_slice() {
                [Value::Int(x), Value::Int(y), Value::UInt(w), Value::UInt(h)] => Some((PhysicalPosition::new(*x as i32, *y as i32), PhysicalSize::new(*w as u32, *h as u32))),
                _ => None,
            },
            _ => None,
        };
        Some(Handoff { frame, face: value.field("face").cloned().unwrap_or(Value::None) })
    }

    pub fn save(&self, window: &Window, face: Value) -> Result<(), String> {
        let frame = match window.outer_position() {
            Ok(p) => {
                let size = window.inner_size();
                Value::List(vec![Value::Int(p.x as i64), Value::Int(p.y as i64), Value::UInt(size.width as u64), Value::UInt(size.height as u64)])
            }
            Err(_) => Value::None,
        };
        let value = Value::Map(vec![("frame".into(), frame), ("face".into(), face)]);
        std::fs::write(&self.handoff, value.to_bytes()).map_err(|e| format!("{}: {e}", self.handoff.display()))
    }
}
