// One byte of the second row is wrong.
pub enum EventKind {
    Deposit,
    Withdraw,
}

// pinoc:discriminators("event:", EventKind)
const DISCRIMINATORS: [[u8; 8]; 2] = [
    [62, 205, 242, 175, 244, 169, 136, 52],
    [193, 241, 201, 217, 70, 150, 90, 247],
];
