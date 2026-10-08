class T { public: T(); T(const T&); T& operator=(const T&); };
T& T::operator=(const T& rhs) { return *new T(rhs); }
T *allocate(T &rhs) { return new T(rhs); }
T *simple() { return new T; }
int *primitive() { return new int(4); }
int *uninit_primitive() { return new int; }
void deleted(T* p) { delete p; }
namespace N { class S { public: S(); S(const S&); }; }
N::S* namespaced(N::S &rhs) { return new N::S(rhs); }
