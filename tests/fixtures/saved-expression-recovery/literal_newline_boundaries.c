int call(const char *);
int baseline(int x) { call("valid"); if (x) ++x; return x; }
int unterminated_string(int x) {
    call("broken
    ++x;
    return x;
}
int unterminated_character(int x) {
    x = 'broken
    ++x;
    return x;
}
int triple_quotes(int x) {
    call(""");
    if (x) ++x;
    return x;
}
int paired_quotes(int x) { call(""""); if (x) ++x; return x; }
int crossing_newline(int x) {
    call("first
second");
    if (x) ++x;
    return x;
}
int quote_condition(int x) {
    if (call(")]'"", x))
        ++x;
    return x;
}
int wide_unterminated(int x) {
    call(L"broken
    if (x) ++x;
    return x;
}
int escaped_newline(int x) {
    call("first\
second");
    if (x) ++x;
    return x;
}
int embedded_quotes(int x) {
    call("#include "config.h"\n#include "stdc.h"\n");
    if (x) ++x;
    return x;
}
int after_literal_problems(int x) { if (x) ++x; return x; }
