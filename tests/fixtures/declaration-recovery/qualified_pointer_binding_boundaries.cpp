typedef unsigned long uintptr_t;
typedef int Scalar;
typedef struct Node { int value; } Record;
typedef struct List { unsigned uxNumberOfItems; } List_t;
int global_object;
int work(int value);

int record_const(int value) { Record *const pointer = (Record *)(uintptr_t)16; return pointer->value; }
int record_volatile(int value) { Record *volatile pointer = (Record *)(uintptr_t)16; return pointer->value; }
int unknown_const(int value) { Unknown *const pointer = (Unknown *)(uintptr_t)16; return value; }
int unknown_volatile(int value) { Unknown *volatile pointer = (Unknown *)(uintptr_t)16; return value; }
int scalar_shadow_const(int Scalar, int value) { Scalar *const pointer = &value; return value; }
int record_shadow_const(int Record, int value) { Record *const pointer = &value; return value; }
int unknown_shadow_const(int Unknown, int value) { Unknown *const pointer = &value; return value; }
int global_shadow_const(int value) { global_object *const pointer = &value; return value; }
int bound_target_const(int Scalar, int value) { Scalar *const value; return value; }
int nested_const(int Record, int value) { Record **const pointer; return value; }
int nested_volatile(int Record, int value) { Record **volatile pointer; return value; }
int plain_object_product(int Record, int value) { Record *value; return value; }
int plain_global_product(int value) { global_object *value; return value; }
int plain_object_assignment(int Record, int value) { Record *value = 2; return value; }
int fresh_pointer(int value) { Record *pointer; return value; }
int cpp_reference_positive(int value) { Record &reference = value; return value; }

int exact_timer_shape(int *const pxListWasEmpty)
{
    List_t *const pxCurrentTimerList =
        *(List_t *const *)(uintptr_t)0x20014720u;
    *pxListWasEmpty = (pxCurrentTimerList->uxNumberOfItems == 0);
    return *pxListWasEmpty;
}

int exact_ready_list_shape(unsigned uxTopPriority)
{
    List_t *const pxList = &((List_t *)(uintptr_t)0x20014504u)[uxTopPriority];
    return pxList->uxNumberOfItems == 0u;
}
