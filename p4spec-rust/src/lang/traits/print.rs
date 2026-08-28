//! Text rendering shared across language stages

use std::fmt;

/// Renders syntax through a shared printer
pub trait Print {
    /// Writes this value using the current printer context
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result;

    /// Renders this value using the default printer context
    fn render(&self) -> String {
        let mut output = String::new();
        {
            let mut printer = Printer::new(&mut output);
            self.print(&mut printer)
                .expect("writing to a String cannot fail");
        }
        output
    }
}

/// Maintains output and layout state while rendering syntax
pub struct Printer<'a> {
    output: &'a mut dyn fmt::Write,
    level: usize,
}

impl<'a> Printer<'a> {
    /// Creates a printer at the outermost indentation level
    pub fn new(output: &'a mut dyn fmt::Write) -> Self {
        Self::with_level(output, 0)
    }

    /// Creates a printer at the given indentation level
    pub fn with_level(output: &'a mut dyn fmt::Write, level: usize) -> Self {
        Self { output, level }
    }

    /// Writes text without changing layout state
    pub fn write(&mut self, text: &str) -> fmt::Result {
        self.output.write_str(text)
    }

    /// Writes formatted arguments without changing layout state
    pub fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        self.output.write_fmt(args)
    }

    /// Writes indentation for the current level
    pub fn write_indent(&mut self) -> fmt::Result {
        for _ in 0..self.level {
            self.output.write_str("  ")?;
        }
        Ok(())
    }

    /// Starts a line at the current indentation level
    pub fn newline(&mut self) -> fmt::Result {
        self.output.write_char('\n')?;
        self.write_indent()
    }

    /// Writes text using OCaml-compatible byte escaping
    pub fn write_escaped(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            match byte {
                b'"' => self.output.write_str("\\\"")?,
                b'\\' => self.output.write_str("\\\\")?,
                8 => self.output.write_str("\\b")?,
                9 => self.output.write_str("\\t")?,
                10 => self.output.write_str("\\n")?,
                13 => self.output.write_str("\\r")?,
                32..=126 => self.output.write_char(char::from(byte))?,
                _ => self.write_fmt(format_args!("\\{byte:03}"))?,
            }
        }
        Ok(())
    }

    /// Renders a nested value one indentation level deeper
    pub fn indented(&mut self, print: impl FnOnce(&mut Self) -> fmt::Result) -> fmt::Result {
        self.level += 1;
        let result = print(self);
        self.level -= 1;
        result
    }

    /// Renders items separated by `sep`
    pub fn separated<T: Print>(&mut self, items: &[T], sep: &str) -> fmt::Result {
        for (index, item) in items.iter().enumerate() {
            if index != 0 {
                self.write(sep)?;
            }
            item.print(self)?;
        }
        Ok(())
    }
}

impl fmt::Write for Printer<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.output.write_str(text)
    }
}
