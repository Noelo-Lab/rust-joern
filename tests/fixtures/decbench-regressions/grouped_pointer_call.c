void serial_begin_write(int arg1)
{
    r0 = arg1;
    r3 = *(r0);
    if (r3 != 0) {
        uint32_t (*r3)() ();
    }
}

void forward_call(int r0)
{
    if (r0 != 0) {
        uint32_t (*r3)(uint32_t, uint32_t) (r0, r3);
    }
}

void single_suffix_call(int r0)
{
    if (r0 != 0) {
        uint32_t (*r3)(uint32_t);
    }
}

void keyword_pointer_declaration(int c)
{
    if (c) {
        int (*callback)(int);
    }
}
