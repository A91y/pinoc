// A table whose rows all match: no finding.
pub enum EventKind {
    Deposit = 0,
    Withdraw = 1,
    Close = 2,
}

// pinoc:discriminators("event:", EventKind)
const DISCRIMINATORS: [[u8; 8]; 3] = [
    [62, 205, 242, 175, 244, 169, 136, 52], // Deposit
    [192, 241, 201, 217, 70, 150, 90, 247], // Withdraw
    [255, 220, 12, 202, 144, 201, 67, 237], // Close
];
