// A row that is not a byte array literal cannot be checked.
pub enum EventKind {
    Deposit,
    Withdraw,
}

const DEPOSIT: [u8; 8] = [62, 205, 242, 175, 244, 169, 136, 52];

// pinoc:discriminators("event:", EventKind)
const DISCRIMINATORS: [[u8; 8]; 2] = [
    DEPOSIT,
    [192, 241, 201, 217, 70, 150, 90, 247],
];
