typedef unsigned int uint32_t;

void typed_pointer_declaration(int c)
{
    if (c) {
        uint32_t (*r3)(uint32_t);
    }
}

void function_returning_function_call(int c)
{
    if (c) {
        uint32_t (*r3)(uint32_t) (r3);
    }
}
