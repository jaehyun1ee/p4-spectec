make
echo "" > spec-gen.include
./watsup p4cherry/spec/* --latex -o spec-gen.include
pdflatex spec-gen.tex
