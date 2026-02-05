package main

import (
	"fmt"
	"math"
)

// Minimal Gorilla XOR encoding demonstration
func main() {
	values := []f64{12.5, 12.5, 12.6, 12.6, 12.8}
	var lastVal uint64
	var first = true

	fmt.Println("Value | XOR Result | Leading Zeros | Trailing Zeros | Decision")
	fmt.Println("----------------------------------------------------------------")

	for _, v := range values {
		curr := math.Float64bits(v)
		if first {
			lastVal = curr
			first = false
			fmt.Printf("%5.1f | (Initial)  | -             | -              | Store full 64 bits\n", v)
			continue
		}

		xor := curr ^ lastVal
		if xor == 0 {
			fmt.Printf("%5.1f | 0x%016x | -             | -              | Store '0'\n", v, xor)
		} else {
			lz := countLeadingZeros(xor)
			tz := countTrailingZeros(xor)
			fmt.Printf("%5.1f | 0x%016x | %-13d | %-14d | Store '1' + Metadata + Meaningful Bits\n", v, xor, lz, tz)
		}
		lastVal = curr
	}
}

func countLeadingZeros(v uint64) int {
	var n int
	for i := 63; i >= 0; i-- {
		if (v >> i) & 1 == 1 {
			break
		}
		n++
	}
	return n
}

func countTrailingZeros(v uint64) int {
	var n int
	for i := 0; i < 64; i++ {
		if (v >> i) & 1 == 1 {
			break
		}
		n++
	}
	return n
}
