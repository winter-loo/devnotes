// Minimal Bit-packing demonstration in Rust
// Concept: Pack a block of 8 values (all < 16) into a single 64-bit integer
// In a real TSDB, this would use SIMD instructions to process multiple blocks.

fn main() {
    // 8 values, each fits in 4 bits (max 15)
    let values: [u32; 8] = [3, 15, 0, 7, 1, 12, 4, 9];
    let bit_width = 4;

    println!("Original Values: {:?}", values);
    
    // Packing
    let mut packed: u64 = 0;
    for (i, &v) in values.iter().enumerate() {
        // Shift value to its position and OR it into the packed u64
        packed |= (v as u64) << (i * bit_width);
    }

    println!("Packed u64 (Hex): 0x{:016x}", packed);
    println!("Bits per value: {}", bit_width);
    println!("Total bits: {} / 64 used", values.len() * bit_width);

    // Unpacking
    let mut unpacked = [0u32; 8];
    let mask = (1 << bit_width) - 1;
    for i in 0..8 {
        unpacked[i] = ((packed >> (i * bit_width)) & mask) as u32;
    }

    println!("Unpacked Values: {:?}", unpacked);
    assert_eq!(values, unpacked);
    println!("Success: Unpacked values match original!");
}
