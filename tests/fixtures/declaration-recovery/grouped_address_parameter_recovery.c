typedef unsigned long Address;
typedef short Small;
extern Small ((Small *)(Address)0x1000)[4];
int after_address(int x) { return (Small)(int)x; }
extern unsigned ((const unsigned *)(Address)0x2000)[16];
int after_builtin(int x) { return (unsigned)(Address)x; }
extern ordinary(int x);
int ordinary(int x) { return x; }
int callback(int (*arg)(int));
int callback(int (*arg)(int)) { return arg(3); }
int (grouped)(int);
int (grouped)(int x) { return x; }
int grouped_parameter(int (x));
int grouped_parameter(int (x)) { return x; }
