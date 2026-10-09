int unknown_alias(int n) { Plain a; signed Plain b; return (Plain)n; }
int unknown_unsigned(int n) { Plain a; unsigned Plain b; if((Plain)n) return 1; return 0; }
int unknown_long(int n) { Plain a; long Plain b; return (Plain)n; }
int unknown_short(int n) { Plain a; short Plain b; return (Plain)n; }
typedef int Alias;
int bound_alias(int n) { signed Alias b; return (Alias)n; }
int bound_unsigned(int n) { unsigned Alias b; if((Alias)n) return 1; return 0; }
int builtin_controls(int n) { signed int a=n; unsigned long b=n; long double c=0; short d=n; return a+b+c+d; }
int real_shadow(int n) { int Alias=n; return (Alias)+1; }
