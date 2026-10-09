int before(int n) { return n; }
typedef struct Broken((packed)) { int field; } Broken;
int after_broken(int n) { return n; }
struct Plain((unused)) { int field; };
int after_plain(int n) { return n; }
typedef struct __attribute__((packed)) Good { int field; } Good;
int valid_record(Good *p) { return p->field; }
struct Suffix __attribute__((packed)) { int field; };
int after_suffix(int n) { return n; }
