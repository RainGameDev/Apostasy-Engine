pub fn flatten(x: u32, y: u32, z: u32, size: u32) -> usize {
    (x + y * size + z * size * size) as usize
}

pub fn unflatten(i: usize, size: u32) -> (u32, u32, u32) {
    let x = i as u32 % size;
    let y = (i as u32 / size) % size;
    let z = i as u32 / (size * size);
    (x, y, z)
}
