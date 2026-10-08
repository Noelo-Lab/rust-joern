typedef char *va_list;
int marker(int value);
long long assignments(va_list ap) {
 long long first = 0; int second = 0;
 first = va_arg(ap, long long);
 second = va_arg(ap, int);
 return first + second;
}
long long return_compound(va_list ap) { return va_arg(ap, long long); }
int return_single(va_list ap) { return va_arg(ap, int); }
void *return_pointer(va_list ap) { return va_arg(ap, char *); }
long long return_unsigned(va_list ap) { return va_arg(ap, unsigned long); }
int condition_compound(va_list ap) { if (va_arg(ap, long long)) marker(1); marker(2); return 3; }
int if_body(va_list ap, int n) { if (n) n = va_arg(ap, long long); else marker(1); marker(2); return n; }
int while_body(va_list ap, int n) { while (n) n = va_arg(ap, long long); marker(2); return n; }
int declaration_initializer(va_list ap) { long long n = va_arg(ap, long long); marker(2); return n; }
int combined_expression(va_list ap, int n) { n += va_arg(ap, long long) + marker(1); marker(2); return n; }
int variadic_not_va_arg(va_list ap) { return arbitrary(ap, long long); }
int no_typename(va_list ap, int n) { return va_arg(ap, n); }
