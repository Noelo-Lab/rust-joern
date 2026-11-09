/* A literal after a complete operand ends CDT's expression: the statement
   lacks its semicolon and the rest is the next statement. Reduced from
   Binary Ninja output, which prints quotes inside a string unescaped. */
int32_t messages(int32_t arg1, char* arg2)
{
    char* format;
    int32_t code;

    if (!arg1)
    {
        format = "empty entry in list "%.100s"";
        code = 0x7c;
    }
    else if (arg1 == 1)
    {
        format = "list entry "%.100s" too long";
        code = 0x89;
    }
    else
    {
        format = "entry "%s" in "%s" ignored";
        code = arg1 + 2;
    }

    report(format, arg2, code);
    return code;
}

int32_t declared(int32_t arg1, char* arg2)
{
    char* format = "domain name "%.100s" too long";
    int32_t code = arg1 5;
    report(format, arg2, code);
    return code;
}

int32_t enclosed(int32_t arg1, char* arg2)
{
    char* format = arg2;

    while (arg1)
        format = "retry "%d" failed";

    if (arg1 > 2)
        *format = "x "%c" y";

    report("path "%s" too long", arg2);
    return arg1;
}

int32_t names(int32_t arg1, char* arg2)
{
    arg2 = "say "hello" now";
    arg1 = arg1 "left" "right";
    return arg1;
}

/* An unclosed literal runs the call into the next statement; CDT's skip
   ends with that statement's brace group. */
int32_t unclosed(int32_t arg1, void* arg2)
{
    if (arg1)
    {
        report(""", arg2);

        if (arg1 > 0)
        {
            arg1 = 1;
        }

        arg1 = arg1 + 3;
    }

    return arg1;
}

/* `T name = *<literal>;` is no declaration to CDT. The expression `T` is a
   statement without its semicolon, and the rest is a problem. */
int32_t registers(int32_t arg1)
{
    if (arg1 > 0x10)
    {
        int32_t r1_43 = *0x3b;
        report("size", r1_43);
    }

    int32_t r0_1 = (*0x98)(arg1, 0x6088c7ec, 0, 2);
    report("node", r0_1);
    return arg1;
}
