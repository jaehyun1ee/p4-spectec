#lang racket/base
;; Mini p4st-in-redex
;;
;; The AL syntax mirrors metasq-example/st-in-st/1-syntax.watsup as
;; s-expressions.
;;
;; A rule reads or writes the context C of the innermost IN around its
;; redex, a rule ((term) C) -> ((term') C') matching
;; (IN C (in-hole F term)), where F is a context without IN.
;; The dynamic determinism check is omitted: ruls-eval takes
;; the first rule that does not unmatch.
(require redex/reduction-semantics
         racket/list
         racket/match)

(provide al ->al run-call)

(define-language al
  ;; AL syntax
  (id string)
  (ctor string)
  (it OPT LIST)
  (val (VBOOL boolean) (VCASE ctor (val ...))
       (VNAT natural) (VTEXT string) (VTUP (val ...)) (VOPT (val ...)) (VLIST (val ...))
       (VEXT string))
  (pat (PCASE ctor) PCONS PNIL PSOME PNONE)
  (exp (EVAR id (it ...)) (EBOOL boolean) (ECMP exp exp) (ECASE ctor (exp ...))
       (ENAT natural) (ETEXT string) (ENE exp exp) (ETUP (exp ...)) (EOPT (exp ...))
       (ELIST (exp ...)) (ECONS exp exp) (ECALL id (exp ...)) (EMATCH exp pat))
  (prem (IF exp) (LET exp exp) (REL id (exp ...) (exp ...)))
  (rul (id (exp ...) (exp ...) (prem ...)))
  (clause ((exp ...) exp (prem ...)))
  (defn (SYND id (ctor ...)) (EXTSYND id)
        (RELD id (rul ...) (rul ...)) (EXTRELD id)
        (DECD id (clause ...) (clause ...)))
  (spec (defn ...))
  ;; Evaluation domains
  (C (CTX spec env))
  (env ((id (it ...) val) ...))
  (fail UNMATCH ERROR)
  ;; Machine terms
  (res (OK (val ...)) OK fail)
  (e res exp prem (IN C e)
     (rel-call id (val ...))
     (ruls-eval (e ...))
     (rul-eval rul (val ...))
     (seq (e ...))
     ;; Values, assignments, outputs of a premise, and meta-function calls
     val
     (assign (exp ...) (val ...)) (exps-eval (e ...)) (rel-ret (exp ...) e)
     (fun-call id (val ...)) (owise e any) (one e))
  ;; Frames. A position still to evaluate is any, as an expression
  ;; evaluates in place and is a val once done.
  (Fr-pass (seq (OK ... hole any ...))
           ;; Premises, expressions, and calls
           (exps-eval (val ... hole any ...))
           (IF hole) (LET any hole) (REL id (val ... hole any ...) any) (rel-ret any hole)
           (ECMP hole any) (ECMP val hole) (ENE hole any) (ENE val hole)
           (ECASE ctor (val ... hole any ...)) (ETUP (val ... hole any ...))
           (EOPT (hole)) (ELIST (val ... hole any ...)) (ECONS hole any) (ECONS val hole)
           (ECALL id (val ... hole any ...)) (EMATCH hole pat)
           (one hole))
  (Fr-catch (ruls-eval (UNMATCH ... hole any ...))
            ;; The otherwise rule or clause, tried once the others unmatch
            (owise hole any))
  ;; Evaluation context; a context C is matched as any, not
  ;; checked against its grammar at every step
  (E hole (IN any E) (in-hole Fr-pass E) (in-hole Fr-catch E))
  ;; An evaluation context within one IN
  (F hole (in-hole Fr-pass F) (in-hole Fr-catch F)))

;;
;; The context
;;

(define (find-defn C id)
  (match C
    [`(CTX ,spec ,_)
     (findf (lambda (defn) (equal? (cadr defn) id)) spec)]))

(define (lookup C id its)
  (match C
    [`(CTX ,_ ,env)
     (define binding (assoc (list id its) (map (lambda (b) (cons (list (car b) (cadr b)) (caddr b))) env)))
     (and binding (cdr binding))]))

;; Binds AL's binding forms, as $assign; #f if the value has another shape
(define (assign C exps vals)
  (cond
    [(not C) #f]
    [(and (null? exps) (null? vals)) C]
    [(or (null? exps) (null? vals)) #f]
    [else (assign (assign-one C (car exps) (car vals)) (cdr exps) (cdr vals))]))

(define (assign-one C exp val)
  (match* (exp val)
    [(`(EVAR ,id ,its) _)
     (match C [`(CTX ,spec ,env) `(CTX ,spec ((,id ,its ,val) ,@env))])]
    [(`(ECASE ,ctor ,exps) `(VCASE ,ctor ,vals)) (assign C exps vals)]
    [(`(ETUP ,exps) `(VTUP ,vals)) (assign C exps vals)]
    [(`(ECONS ,exp_h ,exp_t) `(VLIST (,val_h . ,val_t)))
     (assign-one (assign-one C exp_h val_h) exp_t `(VLIST ,val_t))]
    [(`(EOPT ,exps) `(VOPT ,vals)) (assign C exps vals)]
    [(_ _) #f]))

(define (matches? val pat)
  (match* (val pat)
    [(`(VCASE ,ctor ,_) `(PCASE ,ctor_p)) (equal? ctor ctor_p)]
    [(`(VLIST (,_ . ,_)) 'PCONS) #t]
    [('(VLIST ()) 'PNIL) #t]
    [(`(VOPT (,_)) 'PSOME) #t]
    [('(VOPT ()) 'PNONE) #t]
    [(_ _) #f]))

;; A clause as a rule with one output
(define (rul-of clause)
  (match clause [`(,exps_arg ,exp ,prems) `("" ,exps_arg (,exp) ,prems)]))

;; The extern relation calls; the host sets it
(define host-call-extern-rel
  (make-parameter (lambda (id vals) (error 'redex "no host for extern relation ~a" id))))

;;
;; Rules on a term, in any evaluation context
;;

(define ->term
  (reduction-relation
   al
   #:domain any
   ;; Failures and rule results
   (--> (in-hole E (in-hole Fr-pass fail)) (in-hole E fail) "frame/fail")
   (--> (in-hole E (IN any fail)) (in-hole E fail) "in/fail")
   (--> (in-hole E (ruls-eval (UNMATCH ...))) (in-hole E UNMATCH) "ruls-eval/nil")
   (--> (in-hole E (ruls-eval (UNMATCH ... (OK (val ...)) any ...))) (in-hole E (OK (val ...)))
        "ruls-eval/ok")
   (--> (in-hole E (ruls-eval (UNMATCH ... ERROR any ...))) (in-hole E ERROR) "ruls-eval/error")
   (--> (in-hole E (rul-eval (id (exp_i ...) (exp_o ...) (prem ...)) (val ...)))
        (in-hole E (seq ((assign (exp_i ...) (val ...)) prem ... (exps-eval (exp_o ...)))))
        "rul-eval")
   ;; Leaving an IN and a seq with a result
   (--> (in-hole E (IN any (OK (val ...)))) (in-hole E (OK (val ...))) "in/ok")
   (--> (in-hole E (seq (OK ... (OK (val ...))))) (in-hole E (OK (val ...))) "seq")
   (--> (in-hole E (exps-eval (val ...))) (in-hole E (OK (val ...))) "exps-eval")
   ;; Premises
   (--> (in-hole E (IF (VBOOL #t))) (in-hole E OK) "if/true")
   (--> (in-hole E (IF (VBOOL #f))) (in-hole E UNMATCH) "if/false")
   (--> (in-hole E (LET exp val)) (in-hole E (assign (exp) (val))) "let")
   (--> (in-hole E (REL id (val ...) (exp ...))) (in-hole E (rel-ret (exp ...) (rel-call id (val ...))))
        "rel")
   (--> (in-hole E (rel-ret (exp ...) (OK (val ...)))) (in-hole E (assign (exp ...) (val ...)))
        "rel-ret")
   ;; Expressions, once their subexpressions are values
   (--> (in-hole E (EBOOL boolean)) (in-hole E (VBOOL boolean)) "ebool")
   (--> (in-hole E (ENAT natural)) (in-hole E (VNAT natural)) "enat")
   (--> (in-hole E (ETEXT string)) (in-hole E (VTEXT string)) "etext")
   (--> (in-hole E (ECMP val_l val_r)) (in-hole E (VBOOL ,(equal? (term val_l) (term val_r)))) "ecmp")
   (--> (in-hole E (ENE val_l val_r)) (in-hole E (VBOOL ,(not (equal? (term val_l) (term val_r))))) "ene")
   (--> (in-hole E (ECASE ctor (val ...))) (in-hole E (VCASE ctor (val ...))) "ecase")
   (--> (in-hole E (ETUP (val ...))) (in-hole E (VTUP (val ...))) "etup")
   (--> (in-hole E (EOPT (val ...))) (in-hole E (VOPT (val ...))) "eopt")
   (--> (in-hole E (ELIST (val ...))) (in-hole E (VLIST (val ...))) "elist")
   (--> (in-hole E (ECONS val_h (VLIST (val_t ...)))) (in-hole E (VLIST (val_h val_t ...))) "econs")
   (--> (in-hole E (EMATCH val pat)) (in-hole E (VBOOL ,(matches? (term val) (term pat)))) "ematch")
   (--> (in-hole E (ECALL id (val ...))) (in-hole E (fun-call id (val ...))) "ecall")
   ;; Meta-function results and otherwise rules
   (--> (in-hole E (one (OK (val)))) (in-hole E val) "one")
   (--> (in-hole E (owise (OK (val ...)) any)) (in-hole E (OK (val ...))) "owise/ok")
   (--> (in-hole E (owise UNMATCH ())) (in-hole E UNMATCH) "owise/unmatch")
   (--> (in-hole E (owise UNMATCH (e))) (in-hole E e) "owise/owise")
   (--> (in-hole E (owise ERROR any)) (in-hole E ERROR) "owise/error")))

;;
;; Rules that read or write the context of the innermost IN
;;

(define ->ctx
  (reduction-relation
   al
   #:domain any
   ;; Relation calls
   (--> (in-hole E (IN C (in-hole F (rel-call id (val ...)))))
        (in-hole E (IN C (in-hole F (ruls-eval ((IN C (rul-eval rul (val ...))) ...)))))
        (where (RELD id (rul ...) ()) ,(find-defn (term C) (term id)))
        "rel-call")
   ;; The otherwise rule, and the extern relations
   (--> (in-hole E (IN C (in-hole F (rel-call id (val ...)))))
        (in-hole E (IN C (in-hole F (owise (ruls-eval ((IN C (rul-eval rul (val ...))) ...))
                                           ((IN C (rul-eval rul_o (val ...))))))))
        (where (RELD id (rul ...) (rul_o)) ,(find-defn (term C) (term id)))
        "rel-call/owise")
   (--> (in-hole E (IN C (in-hole F (rel-call id (val ...)))))
        (in-hole E (IN C (in-hole F ,((host-call-extern-rel) (term id) (term (val ...))))))
        (where (EXTRELD id) ,(find-defn (term C) (term id)))
        "rel-call/extern")
   ;; Meta-function calls, as relations with one output
   (--> (in-hole E (IN C (in-hole F (fun-call id (val ...)))))
        (in-hole E (IN C (in-hole F (one (owise (ruls-eval ((IN C (rul-eval rul (val ...))) ...))
                                                ((IN C (rul-eval rul_o (val ...))) ...))))))
        (where (DECD id (clause ...) (clause_o ...)) ,(find-defn (term C) (term id)))
        (where (rul ...) ,(map rul-of (term (clause ...))))
        (where (rul_o ...) ,(map rul-of (term (clause_o ...))))
        "fun-call")
   ;; Variables and assignments
   (--> (in-hole E (IN C (in-hole F (EVAR id (it ...)))))
        (in-hole E (IN C (in-hole F ,(or (lookup (term C) (term id) (term (it ...))) 'ERROR))))
        "evar")
   (--> (in-hole E (IN C (in-hole F (assign (exp ...) (val ...)))))
        (in-hole E ,(let ([C_new (assign (term C) (term (exp ...)) (term (val ...)))])
                      (if C_new (term (IN ,C_new (in-hole F OK))) (term (IN C (in-hole F ERROR))))))
        "assign")))

(define ->al (union-reduction-relations ->term ->ctx))

;; Runs `id` on `vals` under `spec`; returns the final term and the trace of
;; (rule-name . term) steps
(define (run-call spec id vals)
  (let loop ([t `(IN (CTX ,spec ()) (rel-call ,id ,vals))] [steps '()])
    (match (apply-reduction-relation/tag-with-names ->al t)
      ['() (values t (reverse steps))]
      [(list (list name t_next)) (loop t_next (cons (cons name t_next) steps))]
      [nexts (error 'redex "~a steps apply to one term: ~a" (length nexts) (map car nexts))])))
