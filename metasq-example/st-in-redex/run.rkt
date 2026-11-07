#lang racket/base
;; Runs the walk-through on the mini p4st-in-redex specification
;;
;; Usage: racket run.rkt walkthrough.rktd
;; The input is (spec id (val ...) (expected-val ...)), as `metasq-example emit` writes it.
;; Prints the rewrites whenever the outermost ruls-eval changes shape, with C
;; abbreviated by the number of its bindings, then the result against the expected one.
(require racket/match
         racket/list
         racket/pretty
         "st.rkt")

(define (abbrev t)
  (match t
    [`(CTX ,_ ,env) (string->symbol (format "C~a" (length env)))]
    [`(rul-eval (,id . ,_) ,_) `(rul-eval (,id …) …)]
    [`(VBOOL ,b) t]
    [(list (? (lambda (h) (memq h '(VCASE VTUP VLIST VOPT VNAT VTEXT VEXT)))) _ ...) 'v]
    [(list (? (lambda (h) (memq h '(EVAR ECMP ECASE ENE ETUP EOPT ELIST ECONS ECALL EMATCH EBOOL ENAT ETEXT)))) _ ...)
     (list (car t) '…)]
    [`(IF ,e) `(IF ,(abbrev e))]
    [`(LET ,_ ,e) `(LET … ,(abbrev e))]
    [`(REL ,id . ,_) `(REL ,id …)]
    [`(assign . ,_) '(assign …)]
    [`(exps-eval . ,_) '(exps-eval …)]
    [(? list?) (map abbrev t)]
    [_ t]))

;; The shape of the outermost ruls-eval: each rule's progress
(define (shape t)
  (define (elem e)
    (match e
      [`(IN ,C (seq ,es))
       (define-values (oks rest) (splitf-at es (lambda (x) (eq? x 'OK))))
       `(IN ,(abbrev C) (seq (,@oks ,@(if (null? rest) '() (list (head (car rest)) '…)))))]
      [`(IN ,C ,body) `(IN ,(abbrev C) ,(abbrev body))]
      [_ (abbrev e)]))
  (define (head x) (if (pair? x) (list (car x) '…) x))
  (match t
    [`(IN ,_ (ruls-eval ,es)) `(ruls-eval ,(map elem es))]
    [`(IN ,_ ,body) (abbrev body)]
    [_ (abbrev t)]))

(define (main path)
  (match-define (list spec id vals expected) (with-input-from-file path read))
  (define-values (result steps) (run-call spec id vals))
  ;; The rewrites of the call, printed whenever the outermost shape changes;
  ;; the rules between two printed terms are listed after the arrow
  (printf "(rel-call ~s (v_E v_s))\n" id)
  (for/fold ([last-shape #f] [names '()] #:result (void))
            ([step steps])
    (define s (shape (cdr step)))
    (define names* (cons (car step) names))
    (cond
      [(equal? s last-shape) (values s names*)]
      [else
       (define rules (reverse names*))
       (printf "  →~a ~a\n"
               (if (= (length rules) 1) (car rules) (format "* [~a steps, last ~a]" (length rules) (last rules)))
               (pretty-format s 110))
       (values s '())]))
  (printf "steps: ~a\n" (length steps))
  (printf "result: ~a\n" (abbrev result))
  (define ok (equal? result `(OK ,expected)))
  (printf "~a\n" (if ok "result agrees with meta-execution" "RESULT DIFFERS FROM META-EXECUTION"))
  (exit (if ok 0 1)))

(main (vector-ref (current-command-line-arguments) 0))
