pub fn run(mut value: u32) -> u32 {
    while value > 0 {
        for inner in 0..3 {
            if inner == 1 { continue; }
            match value {
                1 => return inner,
                2 => break,
                _ => value -= 1,
            }
        }
        if value > 8 { break; }
        value -= 1;
    }
    value
}
