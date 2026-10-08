import scala.util.{Try, Success, Failure}

def toJson_pp(query: Any, indentLevel: Int = 0): String = {
  val indent = " " * (indentLevel * 4) // 4 spaces for each indent level
  query match {
    case m: Map[_, _] =>
      m.asInstanceOf[Map[String, Any]].map {
        case (k, v) => s"""$indent    "$k": ${toJson_pp(v, indentLevel + 1)}"""
      }.mkString("{\n", ",\n", s"\n$indent}")
    case t: (String, Any) => s"""$indent"$${t._1}": ${toJson_pp(t._2, indentLevel)}"""
    case ss: Seq[_] => ss.map(toJson_pp(_, indentLevel + 1)).mkString("[\n", ",\n", s"\n$indent]")
    case s: String => "\"" + escapeString(s) + "\""
    case null => "null"
    case other => other.toString
  }
}

def escapeString(str: String): String = {
  str
    .replaceAllLiterally("\\", "\\\\") // Escape backslashes
    .replaceAllLiterally("\n", "\\n")  // Escape newlines
    .replaceAllLiterally("\r", "\\r")  // Escape carriage returns
    .replaceAllLiterally("\t", "\\t")  // Escape tabs
    .replaceAllLiterally("\"", "\\\"") // Escape double quotes
}


import io.shiftleft.codepropertygraph.generated.nodes.*
import io.joern.dataflowengineoss.passes.reachingdef.*

@main def capture(target_dir: String) = {
  val cpg = importCode(target_dir)
  run.ossdataflow
  val methods = cpg.method.filter(m => m.lineNumber.isDefined && m.body.typeFullName != "<empty>" && m.name != "<global>").l
  methods.foreach { method =>
    val ns = method.ast.l
    val ids = ns.map(_.id).toSet
    def property(node: StoredNode, key: String): Any = Option(node.propertiesMap.get(key)).getOrElse("")
    val nodes = ns.map { n => Map("id" -> n.id, "kind" -> n.label,
      "code" -> property(n, "CODE").toString, "name" -> property(n, "NAME").toString,
      "type_name" -> property(n, "TYPE_FULL_NAME").toString,
      "method_full_name" -> property(n, "METHOD_FULL_NAME").toString,
      "canonical_name" -> property(n, "CANONICAL_NAME").toString,
      "line" -> n.lineNumber.getOrElse(0), "column" -> n.columnNumber.getOrElse(0),
      "start_byte" -> 0, "end_byte" -> 0,
      "argument_index" -> property(n, "ARGUMENT_INDEX"), "order" -> property(n, "ORDER")) }
    val edges = ns.flatMap { n =>
      List("AST", "ARGUMENT", "CFG", "REF", "PARAMETER_LINK", "REACHING_DEF").flatMap { kind =>
        n.outE(kind).filter(e => ids.contains(e.dst.id)).map { edge =>
          val dst = edge.dst.asInstanceOf[StoredNode]
          val label = if (kind == "ARGUMENT") property(dst, "ARGUMENT_INDEX").toString else if (kind == "REACHING_DEF") Option(edge.property).map(_.toString).getOrElse("") else ""
          Map("source" -> n.id, "target" -> dst.id, "kind" -> kind, "label" -> label)
        }
      }
    }
    val problem = ReachingDefProblem.create(method)
    val solution = new DataFlowSolver().calculateMopSolutionForwards(problem)
    val numberToNode = problem.flowGraph.asInstanceOf[ReachingDefFlowGraph].numberToNode
    val definitions = solution.in.toList.map { case (node, in) =>
      Map("node" -> node.id, "incoming" -> in.toList.map(numberToNode(_).id),
          "outgoing" -> solution.out(node).toList.map(numberToNode(_).id)) }
    val result = Map("name" -> method.name, "fullname" -> method.fullName, "filename" -> method.filename,
      "return_type" -> method.methodReturn.typeFullName, "signature" -> method.signature,
      "start_line" -> method.lineNumber.getOrElse(0), "end_line" -> method.lineNumberEnd.getOrElse(0),
      "cpg" -> Map("nodes" -> nodes, "edges" -> edges),
      "cfg" -> Map("nodes" -> List.empty, "edges" -> List.empty),
      "reaching_definitions" -> definitions)
    println("FLOW_JSON_START")
    println(toJson_pp(result))
    println("FLOW_JSON_END")
  }
}
