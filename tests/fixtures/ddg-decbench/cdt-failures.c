int64_t typed_twice(int64_t arg1, signed int64_t arg2) {
    rdi = arg1;
    return rdi;
}
named ((noreturn)) void named (int message) { message = 0; }
int literal_dereference(int a) {
    *0x40004000 = a;
    if (*0x40004004 == a)
        a = 1;
    for (a = *0x40004008; a < 4; a++)
        a = a + 2;
    return a;
}
int ordinary(int a) { return a; }
word_t (*)(word_t* first) hidden_name()
{
    return 0;
}
int pointer_local(int a) {
    word_t* later;
    later = &a;
    return a;
}
