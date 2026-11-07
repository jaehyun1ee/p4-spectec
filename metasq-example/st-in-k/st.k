// Mini p4st-in-k
//
// The AL syntax mirrors metasq-example/st-in-st/1-syntax.watsup as K
// constructors.
// The dynamic determinism check is omitted: rulsEval takes the
// first rule that does not unmatch.

module ST-SYNTAX
  imports BOOL-SYNTAX
  imports INT-SYNTAX
  imports STRING-SYNTAX

  // An id and a ctor are strings
  syntax It ::= "OPT" | "LIST"
  syntax Its ::= List{It, ","}
  syntax Val ::= VBOOL(Bool) | VCASE(String, Vals)
               | VNAT(Int) | VTEXT(String) | VTUP(Vals) | VOPT(Vals) | VLIST(Vals) | VEXT(String)
  syntax Vals ::= List{Val, ","}
  syntax Pat ::= PCASE(String) | "PCONS" | "PNIL" | "PSOME" | "PNONE"
  syntax Exp ::= EVAR(String, Its) | EBOOL(Bool) | ECMP(Exp, Exp) | ECASE(String, Exps)
               | ENAT(Int) | ETEXT(String) | ENE(Exp, Exp) | ETUP(Exps) | EOPT(Exps)
               | ELIST(Exps) | ECONS(Exp, Exp) | ECALL(String, Exps) | EMATCH(Exp, Pat)
  syntax Exps ::= List{Exp, ","}
  syntax Prem ::= IF(Exp) | LET(Exp, Exp) | REL(String, Exps, Exps)
  syntax Prems ::= List{Prem, ","}
  syntax Rul ::= rul(String, Exps, Exps, Prems)
  syntax Ruls ::= List{Rul, ","}
  syntax Clause ::= clause(Exps, Exp, Prems)
  syntax Clauses ::= List{Clause, ","}
  syntax Ctors ::= List{String, ","}
  syntax Defn ::= SYND(String, Ctors) | EXTSYND(String)
                | RELD(String, Ruls, Ruls) | EXTRELD(String)
                | DECD(String, Clauses, Clauses)
  syntax Defns ::= List{Defn, ","}

  // Parentheses around a list, as `metasq-example emit` prints it
  syntax Its ::= "(" Its ")" [bracket]
  syntax Vals ::= "(" Vals ")" [bracket]
  syntax Exps ::= "(" Exps ")" [bracket]
  syntax Prems ::= "(" Prems ")" [bracket]
  syntax Ruls ::= "(" Ruls ")" [bracket]
  syntax Clauses ::= "(" Clauses ")" [bracket]
  syntax Ctors ::= "(" Ctors ")" [bracket]
  syntax Defns ::= "(" Defns ")" [bracket]

  // The walk-through: run a relation and compare with the expected outputs
  syntax KItem ::= run(Defns, String, Vals, Vals)
endmodule

module ST
  imports ST-SYNTAX
  imports DOMAINS

  // The context: the definitions and the bound meta-variables
  syntax Binding ::= bind(String, Its)
  syntax C ::= CTX(Defns, Map)

  // run sets <ctx>
  configuration <k> $PGM:K </k> <ctx> CTX(.Defns, .Map) </ctx>

  syntax Ok ::= Vals
  syntax Fail ::= "UNMATCH" | "ERROR"
  syntax Res ::= Ok | Fail
  syntax KItem ::= Res

  syntax ErrorBoundary ::= endCall(C)
  syntax UnmatchBoundary ::= ErrorBoundary
                           | afterRul(Ruls, Vals, C)
                           | afterOwise(Ruls, Vals)   // the otherwise rule
  syntax KItem ::= ErrorBoundary | UnmatchBoundary

  rule [unmatch]:
    <k> UNMATCH ~> (K:KItem => .K) ... </k>
    requires notBool isUnmatchBoundary(K)
  rule [error]:
    <k> ERROR ~> (K:KItem => .K) ... </k>
    requires notBool isErrorBoundary(K)
  rule [endCall]:
    <k> W:Res ~> endCall(C) => W ... </k>
    <ctx> _ => C </ctx>

  syntax KItem ::= relCall(String, Vals) | rulsEval(Ruls, Vals) | rulEval(Rul, Vals)

  syntax Defn ::= find(C, String) [function]
  syntax Ruls ::= rulsOf(Defn) [function]
                | owiseOf(Defn) [function]
  syntax Defn ::= "noDefn"
  rule find(CTX((D, _:Defns), _), X) => D
    requires idOf(D) ==String X
  rule find(CTX((D, Ds), M), X) => find(CTX(Ds, M), X)
    requires idOf(D) =/=String X
  rule find(CTX(.Defns, _), _) => noDefn
  syntax String ::= idOf(Defn) [function]
  rule idOf(SYND(X, _)) => X
  rule idOf(EXTSYND(X)) => X
  rule idOf(RELD(X, _, _)) => X
  rule idOf(EXTRELD(X)) => X
  rule idOf(DECD(X, _, _)) => X
  rule rulsOf(RELD(_, Rs, _)) => Rs
  rule owiseOf(RELD(_, _, Rs)) => Rs

  rule [relCall]:
    <k> relCall(X, Vs)
     => rulsEval(rulsOf(find(C, X)), Vs) ~> afterOwise(owiseOf(find(C, X)), Vs) ~> endCall(C) ... </k>
    <ctx> C </ctx>
    requires isRELD(find(C, X))
  rule <k> relCall(X, _) => ERROR ... </k>
       <ctx> C </ctx>
    requires notBool isRELD(find(C, X))
  syntax Bool ::= isRELD(Defn) [function]
  rule isRELD(RELD(_, _, _)) => true
  rule isRELD(_) => false [owise]

  rule [rulsEval/nil]:
    <k> rulsEval(.Ruls, _) => UNMATCH ... </k>
  rule [rulsEval]:
    <k> rulsEval((R:Rul, Rs:Ruls), Vs) => rulEval(R, Vs) ~> afterRul(Rs, Vs, C) ... </k>
    <ctx> C </ctx>
  rule [afterRul/unmatch]:
    <k> UNMATCH ~> afterRul(Rs, Vs, C) => rulsEval(Rs, Vs) ... </k>
    <ctx> _ => C </ctx>
  rule [afterRul/ok]:
    <k> Vs:Vals ~> afterRul(_, _, C) => Vs ... </k>
    <ctx> _ => C </ctx>

  rule [rulEval]:
    <k> rulEval(rul(_, ExpIns, ExpOuts, Ps), Vs)
     => assignExps(ExpIns, Vs) ~> Ps ~> evalExps(ExpOuts) ... </k>

  // The otherwise rule, tried once every other rule unmatches
  rule [afterOwise/unmatch]:
    <k> UNMATCH ~> afterOwise((R:Rul, .Ruls), Vs) => rulEval(R, Vs) ... </k>
  rule [afterOwise/none]:
    <k> UNMATCH ~> afterOwise(.Ruls, _) => UNMATCH ... </k>
  rule [afterOwise/ok]:
    <k> Ws:Vals ~> afterOwise(_, _) => Ws ... </k>

  // Premises in order; each leaves nothing on success
  syntax KItem ::= Prems | ifK() | letK(Exp) | relK(String, Exps) | retK(Exps)
  rule [prems/nil]: <k> .Prems => .K ... </k>
  rule [prems/cons]: <k> (P:Prem, Ps:Prems) => P ~> Ps ... </k>
  rule [if]: <k> IF(E) => E ~> ifK() ... </k>
  rule [if/true]: <k> VBOOL(true) ~> ifK() => .K ... </k>
  rule [if/false]: <k> VBOOL(false) ~> ifK() => UNMATCH ... </k>
  rule [let]: <k> LET(P, E) => E ~> letK(P) ... </k>
  rule [let/val]: <k> V:Val ~> letK(P) => assignExps((P, .Exps), (V, .Vals)) ... </k>
  rule [rel]: <k> REL(X, Ins, Outs) => evalExps(Ins) ~> relK(X, Outs) ... </k>
  rule [rel/call]: <k> Vs:Vals ~> relK(X, Outs) => relCall(X, Vs) ~> retK(Outs) ... </k>
  rule [rel/ret]: <k> Vs:Vals ~> retK(Outs) => assignExps(Outs, Vs) ... </k>

  // Assignment of AL's binding forms to the context
  syntax KItem ::= assignExps(Exps, Vals) | assign(Exp, Val)
  rule <k> assignExps(.Exps, .Vals) => .K ... </k>
  rule <k> assignExps((E, Es), (V, Vs)) => assign(E, V) ~> assignExps(Es, Vs) ... </k>
  rule <k> assign(EVAR(X, Its), V) => .K ... </k>
       <ctx> CTX(_, M => M[bind(X, Its) <- V]) </ctx>
  rule <k> assign(ECASE(Ctor, Es), VCASE(Ctor, Vs)) => assignExps(Es, Vs) ... </k>
  rule <k> assign(ETUP(Es), VTUP(Vs)) => assignExps(Es, Vs) ... </k>
  rule <k> assign(EOPT(Es), VOPT(Vs)) => assignExps(Es, Vs) ... </k>
  rule <k> assign(ECONS(E, Et), VLIST((V, Vs))) => assign(E, V) ~> assign(Et, VLIST(Vs)) ... </k>

  // Expressions, each to a Val on top of <k>
  syntax KItem ::= evalExps(Exps) | evalAcc(Exps, Vals) | evalNext(Exps, Vals)
  rule <k> evalExps(Es) => evalAcc(Es, .Vals) ... </k>
  rule <k> evalAcc(.Exps, Vs) => revVals(Vs, .Vals) ... </k>
  rule <k> evalAcc((E, Es), Vs) => E ~> evalNext(Es, Vs) ... </k>
  rule <k> V:Val ~> evalNext(Es, Vs) => evalAcc(Es, (V, Vs)) ... </k>
  syntax Vals ::= revVals(Vals, Vals) [function]
  rule revVals(.Vals, Acc) => Acc
  rule revVals((V, Vs), Acc) => revVals(Vs, (V, Acc))

  syntax KItem ::= cmpK() | neK() | caseK(String) | tupK() | optK() | listK() | consK()
                 | callK(String) | matchK(Pat) | one()
  rule <k> EVAR(X, Its) => {M[bind(X, Its)]}:>Val ... </k>
       <ctx> CTX(_, M) </ctx>
    requires bind(X, Its) in_keys(M)
  rule <k> EVAR(X, Its) => ERROR ... </k>
       <ctx> CTX(_, M) </ctx>
    requires notBool (bind(X, Its) in_keys(M))
  rule <k> EBOOL(B) => VBOOL(B) ... </k>
  rule <k> ENAT(N) => VNAT(N) ... </k>
  rule <k> ETEXT(S) => VTEXT(S) ... </k>
  rule <k> ECMP(L, R) => evalExps((L, R, .Exps)) ~> cmpK() ... </k>
  rule <k> (V1, V2, .Vals) ~> cmpK() => VBOOL(V1 ==K V2) ... </k>
  rule <k> ENE(L, R) => evalExps((L, R, .Exps)) ~> neK() ... </k>
  rule <k> (V1, V2, .Vals) ~> neK() => VBOOL(V1 =/=K V2) ... </k>
  rule <k> ECASE(Ctor, Es) => evalExps(Es) ~> caseK(Ctor) ... </k>
  rule <k> Vs:Vals ~> caseK(Ctor) => VCASE(Ctor, Vs) ... </k>
  rule <k> ETUP(Es) => evalExps(Es) ~> tupK() ... </k>
  rule <k> Vs:Vals ~> tupK() => VTUP(Vs) ... </k>
  rule <k> EOPT(Es) => evalExps(Es) ~> optK() ... </k>
  rule <k> Vs:Vals ~> optK() => VOPT(Vs) ... </k>
  rule <k> ELIST(Es) => evalExps(Es) ~> listK() ... </k>
  rule <k> Vs:Vals ~> listK() => VLIST(Vs) ... </k>
  rule <k> ECONS(H, T) => evalExps((H, T, .Exps)) ~> consK() ... </k>
  rule <k> (V, VLIST(Vs), .Vals) ~> consK() => VLIST((V, Vs)) ... </k>
  rule <k> EMATCH(E, P) => evalExps((E, .Exps)) ~> matchK(P) ... </k>
  rule <k> (V, .Vals) ~> matchK(P) => VBOOL(matches(V, P)) ... </k>
  rule <k> ECALL(F, Es) => evalExps(Es) ~> callK(F) ... </k>
  rule <k> Vs:Vals ~> callK(F) => funCall(F, Vs) ... </k>

  syntax Bool ::= matches(Val, Pat) [function]
  rule matches(VCASE(Ctor, _), PCASE(Ctor2)) => Ctor ==String Ctor2
  rule matches(VLIST((_, _)), PCONS) => true
  rule matches(VLIST(.Vals), PNIL) => true
  rule matches(VOPT((_, _)), PSOME) => true
  rule matches(VOPT(.Vals), PNONE) => true
  rule matches(_, _) => false [owise]

  // Meta-function calls, as relations with one output
  syntax KItem ::= funCall(String, Vals)
  syntax Ruls ::= rulsOfClauses(Clauses) [function]
  rule rulsOfClauses(.Clauses) => .Ruls
  rule rulsOfClauses((clause(As, E, Ps), Cs)) => (rul("", As, (E, .Exps), Ps), rulsOfClauses(Cs))
  rule <k> funCall(F, Vs)
        => rulsEval(rulsOfClauses(clausesOf(find(C, F))), Vs)
           ~> afterOwise(rulsOfClauses(owiseClausesOf(find(C, F))), Vs) ~> one() ~> endCall(C) ... </k>
       <ctx> C </ctx>
  syntax Clauses ::= clausesOf(Defn) [function] | owiseClausesOf(Defn) [function]
  rule clausesOf(DECD(_, Cs, _)) => Cs
  rule owiseClausesOf(DECD(_, _, Cs)) => Cs
  rule <k> (V, .Vals) ~> one() => V ... </k>
  // The value of a call is a Val, not an Ok, so endCall passes it as well
  rule <k> V:Val ~> endCall(C) => V ... </k>
       <ctx> _ => C </ctx>

  // The walk-through
  syntax KItem ::= expect(Vals)
  syntax KItem ::= "AGREES" | "DIFFERS"
  rule <k> run(Ds, X, Vs, Ws) => relCall(X, Vs) ~> expect(Ws) ... </k>
       <ctx> _ => CTX(Ds, .Map) </ctx>
  rule <k> Ws:Vals ~> expect(Ws) => AGREES ... </k>
  rule <k> Vs:Vals ~> expect(Ws) => DIFFERS ... </k>
    requires Vs =/=K Ws
endmodule
