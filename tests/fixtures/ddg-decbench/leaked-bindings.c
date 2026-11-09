/* Names that CDT binds outside the function that declares them, and the
   `T *x;` statements that then read as products. Reduced from Binary Ninja
   output, whose types have no declaration. */
typedef int known_t;
int64_t file_object;
int32_t declared(int32_t);

int512_t leaks(size_t arg1, size_t arg2)
{
    int64_t rax_1 = arg2 * arg1;

    if ((arg2 * arg1) >> 0x40 != {0})
    {
        int64_t var_10 = rax_1;
        report("leaks: overflow");
    }

    int64_t rax_2;
    int512_t result;
    char* text;
    {
        int64_t inner;
    }
    rax_2 = calloc(arg1, arg2);

    if (!rax_2)
    {
        int64_t nested;
        report("leaks: out of memory");
    }

    *indirect = rax_2;
    counts[0] = rax_1;
    label_1:
    int64_t labelled;
    result = {0};
    return result;
}

int32_t after_leak(int32_t arg1, int32_t* arg2)
{
    int32_t* rax_2;
    int32_t** result;
    int32_t* text;
    int32_t* indirect;
    int32_t* counts;
    int32_t* inner;
    int32_t* nested;
    int32_t* labelled;
    int32_t* rax_3;
    int32_t* rax_1 = arg2;
    rax_2 = arg2;
    rax_3 = rax_1;
    return *rax_2 + *rax_3 + arg1;
}

int32_t file_scope(int32_t arg1, int32_t* arg2)
{
    int32_t* file_object;
    int32_t* declared;
    int32_t* leaks;
    int32_t* file_scope;
    int32_t* later_object;
    int32_t* later_function;
    known_t* typed;
    int32_t* fresh;
    file_object = arg2;
    fresh = arg2;
    typed = arg2;
    return *file_object + *fresh + *typed + arg1;
}

int64_t later_object;

int32_t later_function(int32_t arg1)
{
    return arg1;
}

int64_t external(int64_t arg1, int64_t* arg2)
{
    int64_t* before;
    int64_t var_188;
    int64_t (* var_1b8)[0x29] = &var_188;
    int64_t* after;
    int64_t** twice;
    int32_t* other;
    before = arg2;
    after = before;
    other = &arg1;
    return *after + *other;
}

int64_t after_external(int64_t arg1, int64_t* arg2)
{
    int64_t* rbx_1;
    int64_t* r12 = arg2;
    rbx_1 = r12;
    return *rbx_1 + arg1;
}

int32_t before_function(int32_t arg1, int32_t* arg2)
{
    callback_t* early;
    int32_t* plain;
    early = arg2;
    plain = arg2;
    return *early + *plain + arg1;
}

callback_t (*)(int32_t arg1) handler_for(int32_t arg1)
{
    return 0;
}
