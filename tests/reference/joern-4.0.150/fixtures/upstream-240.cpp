
using namespace std;

class A{
  public:
    int a;
};

class B{
  public:
    A* GetObj();
};

A* B::GetObj() {
  return nullptr;
}

class C{
  public:
    A* GetObject();
};

A* C::GetObject() {
  B * b;
  return b->GetObj();
}

bool Run(A *obj, C *c) {
  const A * a = c->GetObject();
  a->a;
  return true;
}
