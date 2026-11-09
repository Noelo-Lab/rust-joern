typedef struct { int a; } vm_t;
int before(int a) { return a; }
void cast_parameter(int ch, vm_t *(*(const vm_t *)(unsigned long)0x2000dcb8u))
{
    extern float inner_proto(void);
    int x = ch;
    (*(const vm_t *)(unsigned long)0x2000dcb8u)->a = x;
}
int64_t two_names (int64_t arg1, signed int64_t arg2) {
    rdi = arg1;
    return rdi;
}
void bad_token(<anonymous> *** r0, word32 r3)
{
    r3 = **r0;
}
nested_name ((noreturn)) void
nested_name (const char *message, const char *arg)
{
    if (message)
        report (0, 0, message, arg);
}
void reference_parameter(int &a) { a = 1; }
void unknown_qualifier(T U x) { x = 1; }
void bit_field(int a : 3) { a = 1; }
int after_problems(int a) { return a + 1; }
void accepted(int a, struct S { int x; } s, T (*cb)(int, char), int arr[static 3], ...) { a = 1; }
void accepted_abstract(const x, register y, volatile unsigned z, int (*)(void), int (*p)[3]) { x = y; }
void accepted_knr(a, b) int a; int b; { a = b; }
void accepted_attributes(int x __attribute__((unused)), T const *y, T * restrict z) { x = 1; }
void accepted_nested(int a, T (*(*cb)(int))(char, T2 *), void (*)(void (*)(int)), char *(*(*x[3])(void))[5]) { a = 2; }
void accepted_empty(int a, ) { a = 3; }
suffix_first (x, y) void suffix_second (int m) { m = 0; }
split_first ((noreturn))
void split_second (const char *m) { m = 0; }
int typed_first (int x) FOO typed_second (int m) { m = 0; }
FOO int skipped_one(int a) { return a; }
FOO BAR int skipped_two(int a) { return a; }
FOO const BAR skipped_qualified(int a) { return a; }
FOO struct S *skipped_record(int a) { return 0; }
FOO
int skipped_line(int a) { return a; }
implicit_proto(int a);
implicit_definition(int a) { return a; }
static implicit_static(int a);
extern implicit_extern(void), implicit_shared(int);
const implicit_const(int a) { return a; }
*implicit_pointer(int a);
static const **implicit_pointers(int a);
extern unsigned char object_first;
extern unsigned char object_first(int *state);
extern unsigned char function_first(int *state);
extern unsigned char function_first;
int object_list, function_in_list(int), object_list(char);
int object_array[3];
int object_array(int a);
struct record_member { int close; float (*read)(int channel); };
int close(int fd);
T1 (*)(T1* arg1, T2 arg2) swallowed_name()
{
    int rsi = 3;
    return rsi;
}
star_parameter(*p);
static T9 (*pointer_group);
extern void (*hook_function)(void);
void (*hook_defined)(int);
extern struct_0 *hook_pointer;
typedef void (*callback_t)(void);
callback_t hook_typedef;
int hook_array[4];
int calls(int a, void (*parameter)(void)) {
    hook_function(); hook_defined(a); hook_pointer(a); hook_typedef(); hook_array(); parameter();
    return object_first + close(a) + implicit_proto(a);
}
const char *class_prototype(unsigned char class);
int class_parameter(unsigned char class, int new) { int class2 = class; return class2 + new; }
