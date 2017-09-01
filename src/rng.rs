use rand;
use rand::Rng;

pub fn rand_seed() -> [u32; 4] {
    loop {
        let seed = rand::thread_rng().gen::<[u32; 4]>();

        // `rand::XorShiftRng` will panic if the seed is all zeroes.
        // Ensure that does not happen.
        if !(seed[0] == 0 && seed[1] == 0 && seed[2] == 0 && seed[3] == 0) {
            return seed;
        }
    }
}

