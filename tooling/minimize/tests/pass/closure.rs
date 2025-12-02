fn main() {
    let y = 2;
    let z = 3;
    let f = |x| x + y + z;
    assert!(f(1) == 6);
}
