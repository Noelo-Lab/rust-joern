#define BEFORE(x)
int BEFORE(1) before_condition(int x) { return x; }
#if defined(_MSC_VER) || defined(__GNUC__) || defined(_WIN32) || defined(__i386__) || defined(__clang__)
#define API unsupported_vendor_expansion
int excluded_vendor(int x) { while (x) x--; return x; }
#else
#define API
#define ANNOTATE(n)
#endif
int API ANNOTATE(2) annotated(int x) { if (x) return x; return 0; }
#if defined(__STDC__)
int standard_c(int x) { return x + 1; }
#endif
#if defined(__cplusplus)
int selected_cpp(int x) { if (x) x++; return x; }
#else
int selected_c(int x) { return x; }
#endif
#define YES 1
#define NO 0
#if YES && !NO
int defined_source(int x) { return x; }
#endif
#undef YES
#ifdef YES
int excluded_undefined(int x) { return x + 1; }
#else
int undefined_source(int x) { return x; }
#endif
int nested(int x) {
#if 0
    x++;
# if defined(__STDC__)
    while (x) x--;
# endif
#else
# if !(0 || 1) && defined(__STDC__)
    x += 9;
# elif (1 && !0)
    x += 2;
# else
    x += 3;
# endif
#endif
    return x;
}
#define EMPTY()
int body_annotation(int x) { ANNOTATE(x); EMPTY(); return x; }
#define COLLISION(n)
int COLLISION(2) COLLISION(int x, int y) { if (x) return y; return 0; }
int after_collision(int x) { if (x) return x; return 0; }
#undef ANNOTATE
int ANNOTATE(int x) { return x; }
#define ANNOTATE(n)
int ANNOTATE(3) after_redefine(int x) { return x; }
