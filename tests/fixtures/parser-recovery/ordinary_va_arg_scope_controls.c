typedef char *va_list;
typedef int Type;
struct Record { int value; };
int marker(int value);
int alias_argument(va_list ap) { return va_arg(ap, Type); }
int alias_pointer(va_list ap) { return va_arg(ap, Type *); }
int cast_argument(va_list ap, int n) { return va_arg(ap, (long long)n); }
int sizeof_argument(va_list ap) { return va_arg(ap, sizeof(long long)); }
int construct_argument(va_list ap) { return va_arg(ap, int(0)); }
int record_argument(va_list ap) { return va_arg(ap, struct Record); }
int incomplete_argument(va_list ap) { return va_arg(ap, ); }
int brace_if_body(va_list ap, int n) { if (n) { n = va_arg(ap, long long); marker(1); } else marker(2); return n; }
int brace_while_body(va_list ap, int n) { while (n) { n = va_arg(ap, long long); marker(1); } return n; }
int mixed_condition(va_list ap, int n) { if (n && va_arg(ap, long long)) marker(1); return 2; }
int first_type_argument(va_list ap) { return arbitrary(long long, ap); }
int nested_type_argument(va_list ap) { return arbitrary(ap, nested(long long)); }
