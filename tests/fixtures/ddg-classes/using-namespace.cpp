namespace N {
struct A {
  int value;
  virtual ~A();
  int read_a() const;
  int plain_a();
};
struct B {
  int value;
  ~B();
  int read_b() const;
  int plain_b();
};
}
using namespace N;
A::~A() {}
B::~B() {}
int A::read_a() const { return value; }
int A::plain_a() { return value; }
int B::read_b() const { return value; }
int B::plain_b() { return value; }
