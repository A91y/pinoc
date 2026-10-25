// `Withdraw` moved above `Deposit` without its row: both rows are now wrong.
pub enum EventKind {
    Withdraw,
    Deposit,
}

// pinoc:discriminators("event:", EventKind)
const DISCRIMINATORS: [[u8; 8]; 2] = [
    [62, 205, 242, 175, 244, 169, 136, 52],
    [192, 241, 201, 217, 70, 150, 90, 247],
];
