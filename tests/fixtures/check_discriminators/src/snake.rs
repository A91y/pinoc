// Anchor's instruction preimages are snake case.
pub enum Instruction {
    OpenVault = 1,
    CloseVault = 0,
}

// pinoc:discriminators("global:", Instruction, snake_case)
const IX: [[u8; 8]; 2] = [
    [141, 103, 17, 126, 72, 75, 29, 29],
    [181, 248, 228, 67, 6, 175, 37, 167],
];
