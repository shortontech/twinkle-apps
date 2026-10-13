//! Canonical replay vectors, shared across debug/release and CPU architectures.
use twinkle_security::simulation::{Preset, Simulation};
fn main() {
    let mut values = Vec::new();
    for preset in [Preset::Standard, Preset::Dense, Preset::Stress] {
        let mut sim = Simulation::new(42, preset);
        let checkpoints = if preset == Preset::Standard {
            vec![0, 599, 600, 1199, 1200, 1799, 1800, 2399, 2400, 2999, 3000]
        } else {
            vec![0, 3000]
        };
        for tick in checkpoints {
            while sim.tick < tick {
                sim.step();
            }
            values.push(serde_json::json!({"seed":42,"preset":preset.name(),"tick":tick,"checksum":sim.checksum()}));
        }
    }
    values.push(serde_json::json!({"seed":43,"preset":"standard","tick":0,"checksum":Simulation::new(43,Preset::Standard).checksum()}));
    println!("{}", serde_json::to_string_pretty(&values).unwrap());
}
