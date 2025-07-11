use core::mem::MaybeUninit;


struct RingBuffer<T, const SIZE: usize> {
    buffer: [MaybeUninit<T>; SIZE],
    read_pos: usize,
    write_pos: usize,
}
impl<T, const SIZE: usize> RingBuffer<T, SIZE> {

}
