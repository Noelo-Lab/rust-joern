import scala.util.Try
import scala.jdk.CollectionConverters.*
import io.shiftleft.codepropertygraph.generated.nodes.*

def encode(value: Any): String = value match {
  case m: Map[_, _] => m.map { case (k, v) => s"${encode(k.toString)}:${encode(v)}" }.mkString("{", ",", "}")
  case xs: Seq[_] => xs.map(encode).mkString("[", ",", "]")
  case s: String => "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t") + "\""
  case None => "null"
  case Some(x) => encode(x)
  case null => "null"
  case x => x.toString
}

@main def capture(target_dir: String) = {
  val cpg = importCode(target_dir)
  run.ossdataflow
  val methods = cpg.method.filter(m => m.lineNumber.isDefined && m.body.typeFullName != "<empty>" && !m.isExternal && m.name != "<global>").l
  methods.zipWithIndex.foreach { case (method, methodIndex) =>
    {
      def property(node: StoredNode, key: String): Any = Option(node.propertiesMap.get(key)).getOrElse("")
      val ns = method.ast.l
      val ids = ns.map(_.id).toSet
      val nodes = ns.map { n => Map("id" -> n.id, "kind" -> n.label, "properties" -> n.propertiesMap.asScala.toMap) }
      val edges = ns.flatMap { n =>
        List("AST", "ARGUMENT", "CFG", "RECEIVER", "REF", "PARAMETER_LINK", "REACHING_DEF").flatMap { kind =>
          n.outE(kind).filter(e => ids.contains(e.dst.id)).map { edge =>
            Map("source" -> n.id, "target" -> edge.dst.id, "kind" -> kind,
              "label" -> (if (kind == "REACHING_DEF") Option(edge.property).map(_.toString).getOrElse("") else ""))
          }
        }
      }
      val result = Map("method_index" -> methodIndex, "name" -> method.name, "fullname" -> method.fullName,
        "filename" -> method.filename, "return_type" -> method.methodReturn.typeFullName, "signature" -> method.signature,
        "start_line" -> method.lineNumber, "end_line" -> method.lineNumberEnd,
        "code" -> method.code, "is_external" -> method.isExternal,
        "ast_parent_type" -> method.astParentType, "ast_parent_fullname" -> method.astParentFullName,
        "cfg" -> method.dotCfg.l, "ddg" -> method.dotDdg.l,
        "cpg" -> Map("nodes" -> nodes, "edges" -> edges))
      println("CLASS_JSON_START")
      println(encode(result))
      println("CLASS_JSON_END")
    }
  }
}
