int work(int);
struct Global { WRAP(Global) member; int ordinary; int callback(int); };
int local_record(int n) { struct Local { WRAP(Local) member; int ordinary; int callback(int); }; return work(n); }
int after_records(int n) { return work(n); }
