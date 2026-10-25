// A wrong row under an inline allow is not reported.
pub enum EventKind {
    Deposit,
}

// pinoc:discriminators("event:", EventKind)
const DISCRIMINATORS: [[u8; 8]; 1] = [
    // pinoc:allow(DISC001-P)
    [0, 0, 0, 0, 0, 0, 0, 0],
];
