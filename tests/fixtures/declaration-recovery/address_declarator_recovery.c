typedef unsigned long uintptr_t;
typedef struct Record *Alias;
typedef struct Record Profile;
struct Record { int field; };
int work(Alias, int, int);
int address_name(int x) {
  Alias *const (*(Alias *)(uintptr_t)0x1000) = (Alias *)(uintptr_t)0x1000;
  work(*(*(Alias *)(uintptr_t)0x1000), 29, 0);
  return x;
}
int address_profile(int x) {
  Profile **const (*(Profile **)(uintptr_t)0x2000) = (Profile **)(uintptr_t)0x2000;
  (*(Profile **)(uintptr_t)0x2000)->field = x;
  return x;
}
int address_integer(int x) {
  uintptr_t *const (*(uintptr_t *)(uintptr_t)0x3000) = (uintptr_t *)(uintptr_t)0x3000;
  return ((int *)(uintptr_t)0x3000)[x];
}
int pointer_object(int x) {
  Alias *const value = (Alias *)(uintptr_t)0x1000;
  work(*value, 29, 0);
  return x;
}
int grouped_object(int x) {
  Alias *(value) = (Alias *)(uintptr_t)0x1000;
  work(*value, 29, 0);
  return x;
}
int pointer_function(int (*call)(int), int x) { return call(x); }
int shadowed_type(int x) { int Alias=x; return (Alias)+1; }
int after_shadow(int x) { return (*(Profile **)(uintptr_t)0x2000)->field+x; }
