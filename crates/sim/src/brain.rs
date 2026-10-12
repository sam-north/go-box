//! A tiny recurrent neural net: inputs (+ its own memory from last tick) -> tanh hidden -> tanh outputs.
//! The last MEM outputs are fed back in as memory on the next tick.

use crate::sim::{N_RAYS, N_SECTORS};
use rand::Rng;
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

/// v1 brains (before the extra senses and the bigger hidden layer); upgraded on load.
const V1_IN: usize = IN_EXTRA;
const V1_HID: usize = 32;
const V1_WEIGHTS: usize = (V1_IN + 1) * V1_HID + (V1_HID + 1) * N_OUT;
/// move, run, jump, attack, throw, aim, grab, memory...
pub const N_OUT: usize = 7 + MEM;
pub const OUT_MEM: usize = 7;

#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "SavedBrain")]
pub struct Brain {
    pub w: Vec<f32>,
}

/// A brain as stored on disk, which may be an older, smaller layout.
#[derive(Deserialize)]
struct SavedBrain {
    w: Vec<f32>,
}

impl From<SavedBrain> for Brain {
    /// Grafts v1 brains onto the current layout: new inputs and new hidden neurons get zero
    /// weights, so the brain behaves exactly as before and evolution can grow into the extra
    /// capacity. Anything else is kept as-is (and rejected later by `is_current`).
    fn from(s: SavedBrain) -> Self {
        if s.w.len() != V1_WEIGHTS {
            return Self { w: s.w };
        }
        let mut w = vec![0.0; Self::N_WEIGHTS];
        for j in 0..V1_HID {
            let old = &s.w[j * (V1_IN + 1)..(j + 1) * (V1_IN + 1)];
            let new = &mut w[j * (N_IN + 1)..(j + 1) * (N_IN + 1)];
            new[..V1_IN].copy_from_slice(&old[..V1_IN]);
            new[N_IN] = old[V1_IN];
        }
        let (ob, nb) = ((V1_IN + 1) * V1_HID, (N_IN + 1) * N_HID);
        for k in 0..N_OUT {
            let old = &s.w[ob + k * (V1_HID + 1)..ob + (k + 1) * (V1_HID + 1)];
            let new = &mut w[nb + k * (N_HID + 1)..nb + (k + 1) * (N_HID + 1)];
            new[..V1_HID].copy_from_slice(&old[..V1_HID]);
            new[N_HID] = old[V1_HID];
        }
        Self { w }
    }
}

pub fn gauss(rng: &mut impl Rng) -> f32 {
    let u1: f32 = rng.r#gen::<f32>().max(1e-7);
    let u2: f32 = rng.r#gen();
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}

impl Brain {
    pub const N_WEIGHTS: usize = (N_IN + 1) * N_HID + (N_HID + 1) * N_OUT;

    pub fn random(rng: &mut impl Rng) -> Self {
        let mut w = Vec::with_capacity(Self::N_WEIGHTS);
        let s1 = 1.5 / (N_IN as f32).sqrt();
        let s2 = 1.5 / (N_HID as f32).sqrt();
        for _ in 0..(N_IN + 1) * N_HID {
            w.push(gauss(rng) * s1);
        }
        for _ in 0..(N_HID + 1) * N_OUT {
            w.push(gauss(rng) * s2);
        }
        Self { w }
    }

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

    /// Every brain (`*.json`) in `dir`, in file-name order.
    pub fn load_dir(dir: &std::path::Path) -> Result<Vec<Self>, String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("can't read {}: {e}", dir.display()))?;
        let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        paths.sort();
        paths.iter().map(|p| Self::load(p)).collect()
    }

    /// The champions committed in the repo's `brains/` (for tests).
    #[cfg(test)]
    pub(crate) fn committed() -> Vec<Self> {
        Self::load_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../brains")).unwrap()
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

#[cfg(test)]
mod graft_tests {
    use super::*;

    /// v1 forward pass, as it was before the upgrade.
    fn forward_v1(w: &[f32], input: &[f32], out: &mut [f32; N_OUT]) {
        let mut hid = [0.0f32; V1_HID];
        for (j, h) in hid.iter_mut().enumerate() {
            let row = &w[j * (V1_IN + 1)..(j + 1) * (V1_IN + 1)];
            *h = tanh(row[V1_IN] + row[..V1_IN].iter().zip(input).map(|(a, b)| a * b).sum::<f32>());
        }
        let base = (V1_IN + 1) * V1_HID;
        for (k, o) in out.iter_mut().enumerate() {
            let row = &w[base + k * (V1_HID + 1)..base + (k + 1) * (V1_HID + 1)];
            *o = tanh(row[V1_HID] + row[..V1_HID].iter().zip(&hid).map(|(a, b)| a * b).sum::<f32>());
        }
    }

    #[test]
    fn grafted_brain_behaves_like_the_original() {
        let mut rng = rand::rngs::mock::StepRng::new(1, 0x9E37_79B9_7F4A_7C15);
        let old: Vec<f32> = (0..V1_WEIGHTS).map(|_| gauss(&mut rng) * 0.3).collect();
        let grafted = Brain::from(SavedBrain { w: old.clone() });
        assert!(grafted.is_current());
        let mut input = [0.0f32; N_IN];
        for (i, v) in input.iter_mut().enumerate() {
            *v = ((i * 37 % 11) as f32 - 5.0) / 5.0;
        }
        let (mut a, mut b) = ([0.0; N_OUT], [0.0; N_OUT]);
        forward_v1(&old, &input[..V1_IN], &mut a);
        grafted.forward(&input, &mut b);
        for k in 0..N_OUT {
            assert!((a[k] - b[k]).abs() < 1e-4, "output {k}: {} vs {}", a[k], b[k]);
        }
    }
}
