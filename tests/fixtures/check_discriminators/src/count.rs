// A variant was added and its row was not.
pub enum EventKind {
    Deposit,
    Withdraw,
    Close,
}

// pinoc:discriminators("event:", EventKind)
const DISCRIMINATORS: [[u8; 8]; 2] = [
    [62, 205, 242, 175, 244, 169, 136, 52],
    [192, 241, 201, 217, 70, 150, 90, 247],
];
