//! A tiny recurrent neural net: inputs (+ its own memory from last tick) -> tanh hidden -> tanh outputs.
//! The last MEM outputs are fed back in as memory on the next tick.

use crate::sim::{N_RAYS, N_SECTORS};
use rand::Rng;
use serde::{Deserialize, Serialize};

pub const MEM: usize = 4;

// Input layout. Each version only appends inputs, so an older brain's inputs are always a
// prefix of the current layout and can be grafted in place.
//   v1: rays | enemy retina | enemy swinging, enemy facing me | 7 self-state | memory
//   v2: ally retina | threat retina (projectiles) | loose-rock retina | holding rock, ranged ready, enemy is ranged
//   v3: health-pickup retina
pub const IN_MEM: usize = N_RAYS + N_SECTORS + 2 + 7;
pub const IN_ALLY: usize = IN_MEM + MEM;
pub const IN_THREAT: usize = IN_ALLY + N_SECTORS;
pub const IN_ROCK: usize = IN_THREAT + N_SECTORS;
pub const IN_EXTRA: usize = IN_ROCK + N_SECTORS;
pub const IN_HEAL: usize = IN_EXTRA + 3;
pub const N_IN: usize = IN_HEAL + N_SECTORS;
pub const N_HID: usize = 28;
/// move, run, jump, melee, ranged, aim, memory...
pub const N_OUT: usize = 6 + MEM;
pub const OUT_MEM: usize = 6;

/// Shape of an older brain version, for migrating saves.
struct Legacy {
    n_in: usize,
    n_hid: usize,
    n_out: usize,
    /// Which old output feeds each current output (None = new output).
    out_from: fn(usize) -> Option<usize>,
}

impl Legacy {
    const fn weights(&self) -> usize {
        (self.n_in + 1) * self.n_hid + (self.n_hid + 1) * self.n_out
    }
}

const LEGACY: [Legacy; 2] = [
    // v1 outputs: move, run, jump, attack, mem*4 (no ranged/aim).
    Legacy { n_in: IN_ALLY, n_hid: 20, n_out: 4 + MEM, out_from: |k| if k < 4 { Some(k) } else if k >= OUT_MEM { Some(k - 2) } else { None } },
    Legacy { n_in: IN_HEAL, n_hid: 28, n_out: N_OUT, out_from: Some },
];

#[derive(Clone, Serialize, Deserialize)]
pub struct Brain {
    pub w: Vec<f32>,
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

    pub fn is_legacy(&self) -> bool {
        LEGACY.iter().any(|l| l.weights() == self.w.len())
    }

    /// Converts an older brain to the current shape without changing its behaviour: new
    /// senses and new hidden neurons start disconnected from the outputs, so evolution can
    /// wire them in. Brand-new outputs get small random weights so they get explored.
    /// Returns the brain unchanged if it is already current.
    pub fn upgrade(&self, rng: &mut impl Rng) -> Brain {
        let Some(old) = LEGACY.iter().find(|l| l.weights() == self.w.len()) else {
            return self.clone();
        };
        let mut w = vec![0.0f32; Self::N_WEIGHTS];
        for j in 0..N_HID {
            let row = j * (N_IN + 1);
            if j < old.n_hid {
                let old_row = j * (old.n_in + 1);
                w[row..row + old.n_in].copy_from_slice(&self.w[old_row..old_row + old.n_in]);
                w[row + N_IN] = self.w[old_row + old.n_in];
            } else {
                for i in 0..=N_IN {
                    w[row + i] = gauss(rng) * 0.1;
                }
            }
        }
        let (base, old_base) = ((N_IN + 1) * N_HID, (old.n_in + 1) * old.n_hid);
        for k in 0..N_OUT {
            let row = base + k * (N_HID + 1);
            match (old.out_from)(k) {
                Some(ok) => {
                    let old_row = old_base + ok * (old.n_hid + 1);
                    w[row..row + old.n_hid].copy_from_slice(&self.w[old_row..old_row + old.n_hid]);
                    w[row + N_HID] = self.w[old_row + old.n_hid];
                }
                None => {
                    for j in 0..=N_HID {
                        w[row + j] = gauss(rng) * 0.3;
                    }
                }
            }
        }
        Brain { w }
    }

    pub fn forward(&self, input: &[f32; N_IN], out: &mut [f32; N_OUT]) {
        let mut hid = [0.0f32; N_HID];
        for (j, h) in hid.iter_mut().enumerate() {
            let row = &self.w[j * (N_IN + 1)..(j + 1) * (N_IN + 1)];
            let mut acc = row[N_IN];
            for i in 0..N_IN {
                acc += row[i] * input[i];
            }
            *h = acc.tanh();
        }
        let base = (N_IN + 1) * N_HID;
        for (k, o) in out.iter_mut().enumerate() {
            let row = &self.w[base + k * (N_HID + 1)..base + (k + 1) * (N_HID + 1)];
            let mut acc = row[N_HID];
            for j in 0..N_HID {
                acc += row[j] * hid[j];
            }
            *o = acc.tanh();
        }
    }
}
