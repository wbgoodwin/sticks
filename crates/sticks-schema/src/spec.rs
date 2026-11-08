//! The `spec!` macro: a schema struct whose fields mirror a core type's, each
//! optional, with the conversion both ways.
//!
//! Each field is marked `fill` or `keep`:
//!
//! - `fill`: the core field always has a value. Left out of the design, it
//!   takes the base value (the core type's default for a new design) and the
//!   choice is noted as an assumption.
//! - `keep`: the core field is itself optional and already means "Sticks
//!   works it out", so it passes straight through.
//!
//! Fields listed under `extra` are the schema's own (ids, required sizes);
//! `to_core` leaves them to the caller and `from_core` takes them as
//! arguments.

use serde::Serialize;

use crate::validate::Note;

/// Assumptions gathered while filling a design in.
#[derive(Default)]
pub(crate) struct Notes {
    pub assumed: Vec<Note>,
}

impl Notes {
    pub fn assume<T: Serialize>(&mut self, path: String, value: &T) {
        let shown = match serde_json::to_value(value) {
            Ok(serde_json::Value::String(s)) => s,
            Ok(v) => v.to_string(),
            Err(_) => return,
        };
        self.assumed.push(Note {
            message: format!("not given, so {shown}"),
            path,
        });
    }

    pub fn say(&mut self, path: String, message: String) {
        self.assumed.push(Note { path, message });
    }
}

macro_rules! spec {
    (
        $(#[$sm:meta])*
        pub struct $Spec:ident => $Core:ty {
            extra {
                $( $(#[$em:meta])* $ef:ident : $et:ty, )*
            }
            $( $(#[$fm:meta])* $how:ident $f:ident : $t:ty, )*
        }
    ) => {
        $(#[$sm])*
        #[derive(Clone, Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $Spec {
            $( $(#[$em])* pub $ef: $et, )*
            $(
                $(#[$fm])*
                #[serde(default, skip_serializing_if = "Option::is_none")]
                pub $f: Option<$t>,
            )*
        }

        impl $Spec {
            /// The core value: `base` with every field this spec gives
            /// written over it. `extra` fields are the caller's to set.
            #[allow(unused_variables)]
            pub(crate) fn to_core(
                &self,
                base: $Core,
                path: &str,
                notes: &mut $crate::spec::Notes,
            ) -> $Core {
                let mut c = base;
                $( spec!(@to $how, c, self, $f, path, notes); )*
                c
            }

            /// The spec for a core value, every field spelled out.
            pub(crate) fn from_core(c: &$Core, $( $ef: $et, )*) -> Self {
                Self {
                    $( $ef, )*
                    $( $f: spec!(@from $how, c, $f), )*
                }
            }
        }
    };

    (@to fill, $c:ident, $s:ident, $f:ident, $path:ident, $notes:ident) => {
        match &$s.$f {
            Some(v) => $c.$f = v.clone(),
            None => $notes.assume(format!("{}.{}", $path, stringify!($f)), &$c.$f),
        }
    };
    (@to keep, $c:ident, $s:ident, $f:ident, $path:ident, $notes:ident) => {
        $c.$f = $s.$f.clone();
    };
    (@from fill, $c:ident, $f:ident) => {
        Some($c.$f.clone())
    };
    (@from keep, $c:ident, $f:ident) => {
        $c.$f.clone()
    };
}

pub(crate) use spec;
