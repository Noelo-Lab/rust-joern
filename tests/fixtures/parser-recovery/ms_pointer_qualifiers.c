typedef struct Record { int value; } Record;
extern Record known_object;
int g(int x);
void *ms_unknown_cast32(void *p) { p = (_UNKNOWN *__ptr32 *)&unk_12E983; return p; }
void *ms_unknown_cast64(void *p) { p = (_UNKNOWN *__ptr64 *)&unk_12E983; return p; }
void *ms_known_cast32(void *p) { p = (Record *__ptr32 *)&known_object; return p; }
void *ms_known_cast64(void *p) { p = (Record *__ptr64 *)&known_object; return p; }
void *ms_ordinary_cast(void *p) { p = (Record **)&known_object; return p; }
int ms_cast_if(void *p, int x) { if (x) p = (_UNKNOWN *__ptr32 *)&unk_12E983; return x; }
int ms_cast_condition(void *p, int x) { if ((_UNKNOWN *__ptr32 *)&unk_12E983) g(x); return x; }
int ms_decl32(void *p, int x) { int *__ptr32 pointer = p; g(x); return x; }
int ms_decl64(void *p, int x) { int *__ptr64 pointer = p; g(x); return x; }
int ms_decl32_nested(void *p, int x) { int *__ptr32 *pointer = p; g(x); return x; }
int ms_qualifier_identifier(int x) { __ptr32(x); __ptr64(x); return x; }
int ms_unknown_qualifier(void *p, int x) { p = (Record *__madeup *)&known_object; return x; }
int ms_ptr32_before_type(void *p, int x) { __ptr32 int *pointer = p; return x; }
