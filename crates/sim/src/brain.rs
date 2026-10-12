//! A tiny recurrent neural net: inputs (+ its own memory from last tick) -> tanh hidden -> tanh outputs.
//! The last MEM outputs are fed back in as memory on the next tick.

use crate::sim::{N_RAYS, N_SECTORS};
use serde::{Deserialize, Serialize};

pub const MEM: usize = 4;

// Input layout:
//   terrain rays | retinas: opponents, threats (flying items, live grenades), loose spears,
//   loose rocks, loose grenades, health | nearest opponent (5) | self (13) | memory
pub const IN_OPP: usize = N_RAYS;
pub const IN_THREAT: usize = IN_OPP + N_SECTORS;
/// Loose-item retinas, one per `ItemKind` in order (spear, rock, grenade).
pub const IN_ITEMS: usize = IN_THREAT + N_SECTORS;
pub const IN_HEAL: usize = IN_ITEMS + 3 * N_SECTORS;
pub const IN_NEAREST: usize = IN_HEAL + N_SECTORS;
pub const IN_SELF: usize = IN_NEAREST + 5;
pub const IN_MEM: usize = IN_SELF + 13;
/// Added in v2 (appended so v1 input indices are unchanged): nearest opponent's health,
/// relative position and velocity (5), opponents alive (1), incoming projectile danger, time to
/// impact and direction (4), grenade blast danger (1), time left and health lead (2).
pub const IN_EXTRA: usize = IN_MEM + MEM;
pub const N_EXTRA: usize = 13;
pub const N_IN: usize = IN_EXTRA + N_EXTRA;
pub const N_HID: usize = 64;

/// move, run, jump, attack, throw, aim, grab, memory...
pub const N_OUT: usize = 7 + MEM;
pub const OUT_MEM: usize = 7;

#[derive(Clone, Serialize, Deserialize)]
pub struct Brain {
    pub w: Vec<f32>,
}

impl Brain {
    pub const N_WEIGHTS: usize = (N_IN + 1) * N_HID + (N_HID + 1) * N_OUT;

    pub fn is_current(&self) -> bool {
        self.w.len() == Self::N_WEIGHTS
    }

    /// A brain saved as JSON (as in `brains/`), checked to fit this network.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let brain: Brain = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if !brain.is_current() {
            return Err(format!("wrong shape: {} weights, expected {}", brain.w.len(), Self::N_WEIGHTS));
        }
        Ok(brain)
    }

    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("can't read brain {}: {e}", path.display()))?;
        Self::from_json(&text).map_err(|e| format!("bad brain {}: {e}", path.display()))
    }

    /// Every brain (`*.json`) in `dir`, with its file name, in file-name order.
    pub fn load_dir(dir: &std::path::Path) -> Result<Vec<(String, Self)>, String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("can't read {}: {e}", dir.display()))?;
        let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        paths.sort();
        paths.iter().map(|p| Ok((p.file_name().unwrap_or_default().to_string_lossy().into_owned(), Self::load(p)?))).collect()
    }

    /// The champions committed in the repo's `brains/` (for tests).
    #[cfg(test)]
    pub(crate) fn committed() -> Vec<Self> {
        Self::load_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../brains")).unwrap().into_iter().map(|(_, b)| b).collect()
    }

    pub fn forward(&self, input: &[f32; N_IN], out: &mut [f32; N_OUT]) {
        let mut hid = [0.0f32; N_HID];
        for (j, h) in hid.iter_mut().enumerate() {
            let row = &self.w[j * (N_IN + 1)..(j + 1) * (N_IN + 1)];
            *h = tanh(row[N_IN] + dot(&row[..N_IN], input));
        }
        let base = (N_IN + 1) * N_HID;
        for (k, o) in out.iter_mut().enumerate() {
            let row = &self.w[base + k * (N_HID + 1)..base + (k + 1) * (N_HID + 1)];
            *o = tanh(row[N_HID] + dot(&row[..N_HID], &hid));
        }
    }
}

/// Dot product with 8 independent accumulators, so the compiler can use SIMD (a plain running
/// sum has to be added strictly in order, which blocks vectorisation).
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let (ac, bc) = (a.chunks_exact(8), b.chunks_exact(8));
    let tail: f32 = ac.remainder().iter().zip(bc.remainder()).map(|(x, y)| x * y).sum();
    let mut acc = [0.0f32; 8];
    for (x, y) in ac.zip(bc) {
        for l in 0..8 {
            acc[l] += x[l] * y[l];
        }
    }
    acc.iter().sum::<f32>() + tail
}

/// Fast tanh: a rational approximation (as used by Eigen), accurate to ~1e-6 and several times
/// quicker than `f32::tanh`.
pub fn tanh(x: f32) -> f32 {
    let x = x.clamp(-7.905_311, 7.905_311);
    let x2 = x * x;
    let mut p = -2.760_768_5e-16f32;
    p = p * x2 + 2.000_187_9e-13;
    p = p * x2 - 8.604_671_5e-11;
    p = p * x2 + 5.122_297e-8;
    p = p * x2 + 1.485_722_4e-5;
    p = p * x2 + 6.372_619_3e-4;
    p = p * x2 + 4.893_524_6e-3;
    let q = ((1.198_258_4e-6 * x2 + 1.185_347_1e-4) * x2 + 2.268_434_6e-3) * x2 + 4.893_525e-3;
    x * p / q
}

#[cfg(test)]
mod tests {
    #[test]
    fn fast_tanh_matches_std() {
        let worst = (-2000..=2000).map(|i| i as f32 / 100.0).map(|x| (super::tanh(x) - x.tanh()).abs()).fold(0.0, f32::max);
        assert!(worst < 1e-5, "max error {worst}");
    }
}
