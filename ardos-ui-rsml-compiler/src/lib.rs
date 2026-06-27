//! # RSML (RuSt Markup Language) Compiler
//!
//! A DOM-based compiler that transforms JSX-like syntax into Ardos UI Rust code.
//!
//! ## Architecture Overview
//!
//! The compiler follows a token-based pipeline that preserves spans:
//! 1. **Tokenization**: `TokenStream` → RSML tokens (with spans)
//! 2. **Parsing**: RSML tokens → DOM tree
//! 3. **Code Generation**: DOM tree → Rust token stream
//!
//! ## Notes on Diagnostics
//!
//! This crate prefers returning `syn::Error` with spans so errors point to the
//! specific RSML location instead of failing the whole macro invocation.
//!
//! ## Example Transformation
//!
//! Input RSML:
//! ```rsml
//! <container padding_all={16} center>
//!     <text font_size={18}>Hello World!</text>
//!     <MyComponent name="test" active />
//! </container>
//! ```
//!
//! Output Rust:
/// ```rust,ignore
/// Box::new(ardos_ui::Container::new().padding_all(16).center()
///     .child(Box::new(ardos_ui::Text::new("Hello World!").font_size(18)))
///     .child(ardos_ui::Component::new(MyComponent, {
///         let mut props = Default::default();
///         props.name = "test";
///         props.active = true;
///         props
///     })))
/// ```
use proc_macro::TokenStream;

use proc_macro2::{Delimiter, Span, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use std::collections::HashMap;
use std::sync::LazyLock;
use syn::Ident;

/// Wrap a Rust expression token stream in Ardos UI's identity macro.
///
/// This is intended to improve editor tooling behavior for expressions originating
/// inside RSML `{ ... }` placeholders.
fn wrap_rsml_expr(ts: TokenStream2) -> TokenStream2 {
	quote! { ardos_ui::__rsml_expr!(#ts) }
}

/// Wrap a boolean-ish Rust expression token stream in Ardos UI's identity macro.
///
/// This is intended to improve editor tooling behavior for boolean-driven
/// attribute application paths.
fn wrap_rsml_bool(ts: TokenStream2) -> TokenStream2 {
	quote! { ardos_ui::__rsml_bool!(#ts) }
}

/// Simple spanned wrapper used across the tokenizer/parser/codegen pipeline.
///
/// Note: we intentionally do **not** derive `PartialEq`/`Eq` because `Span` does
/// not implement those traits.
#[derive(Debug, Clone)]
struct Spanned<T> {
	value: T,
	span: Span,
}

impl<T> Spanned<T> {
	fn new(value: T, span: Span) -> Self {
		Self { value, span }
	}
}

fn collapse_html_whitespace(input: &str) -> String {
	let mut out = String::new();
	let mut in_ws = false;

	for ch in input.chars() {
		if ch.is_whitespace() {
			in_ws = true;
			continue;
		}

		if in_ws && !out.is_empty() {
			out.push(' ');
		}
		in_ws = false;
		out.push(ch);
	}

	out.trim().to_string()
}

// ============================================================================
// DOM DATA STRUCTURES
// ============================================================================

/// A node in the RSML DOM tree.
///
/// The DOM represents the parsed structure before code generation.
/// This allows for easy inspection, transformation, and debugging.
#[derive(Debug, Clone)]
enum Node {
	/// An HTML-like element: `<tag attr="value">children</tag>`
	Element(Element),
	/// Plain text content between tags: `Hello World`
	Text(Spanned<String>),
	/// Rust tokens in braces: `{ some_rust_expr }`
	Expression(Spanned<TokenStream2>),
}

/// An RSML element with tag name, attributes, and children.
///
/// Examples:
/// - `<container />` - self-closing with no attributes
/// - `<text font_size={16}>Hello</text>` - with attributes and text content
/// - `<MyComponent prop="value">...</MyComponent>` - component with children
#[derive(Debug, Clone)]
struct Element {
	/// The tag name (e.g., "container", "text", "MyComponent")
	tag_name: Spanned<String>,
	/// All attributes on the element
	attributes: Vec<Attribute>,
	/// Child nodes (other elements, text, or expressions)
	children: Vec<Node>,
	/// Whether this is a self-closing tag like `<container />`
	self_closing: bool,
}

/// An attribute on an RSML element.
///
/// Examples:
/// - `disabled` - boolean attribute (no value)
/// - `name="John"` - string literal value
/// - `size={42}` - expression value
#[derive(Debug, Clone)]
struct Attribute {
	/// The attribute name
	name: Spanned<String>,
	/// The attribute value (if any)
	value: Option<AttributeValue>,
}

/// The value of an attribute.
#[derive(Debug, Clone)]
enum AttributeValue {
	/// String literal: `name="value"`
	String(Spanned<String>),
	/// Rust tokens: `size={variable + 1}`
	Expression(Spanned<TokenStream2>),
}

// ============================================================================
// TOKEN-TREE PARSER (SPAN-PRESERVING)
// ============================================================================

/// A token kind in the RSML token stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
	/// Opening tag bracket: `<`
	OpenTag,
	/// Closing tag bracket: `>`
	CloseTag,
	/// Self-closing tag: `/>`
	SelfCloseTag,
	/// End tag opening: `</`
	EndOpenTag,
	/// Equals sign for attributes: `=`
	Equals,
	/// End of input
	Eof,
	/// Identifier: tag names, attribute names, etc. (only valid inside tags)
	Identifier,
	/// String literal in Rust form (e.g. `"hello"`). Stored as decoded string.
	StringLiteral,
	/// Rust tokens in braces: `{ ... }`
	Expression,
	/// Raw text content between tags (JSX-style text nodes)
	Text,
}

/// A token with an attached span and optional payload.
///
/// This crate is token-based: we do not stringify and re-parse RSML in the proc-macro path.
#[derive(Debug, Clone)]
struct Token {
	kind: TokenKind,
	span: Span,
	payload: Option<String>,
	/// For `{ ... }` expressions, preserve the original token stream.
	expr_tokens: Option<TokenStream2>,
}

// ============================================================================
// CODE GENERATOR
// ============================================================================

/// Generates Rust tokens from a DOM tree.
///
/// The code generator traverses the DOM and produces idiomatic Ardos UI Rust code as
/// a `TokenStream2`, preserving spans where possible.
///
/// It handles:
/// - Built-in elements (container, text) → Element constructors
/// - Components (uppercase tags) → Component::new with props
/// - Attributes → Method calls or prop assignments
/// - Children → .child() calls or props.children vector
struct CodeGenerator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BooleanAttrKind {
	/// 0-arg flag method (e.g. `.center()`):
	/// - `flag` => `.flag()`
	/// - `flag={expr}` => `if expr { .flag() } else { identity }`
	FlagMethod,
	/// Bool-parameter toggle method (e.g. `.floating(bool)`):
	/// - `flag` => `.flag(true)`
	/// - `flag={expr}` => `.flag(expr)`
	ToggleBoolParam,
}

static BOOLEAN_ATTR_RULES: LazyLock<HashMap<(&'static str, &'static str), BooleanAttrKind>> =
	LazyLock::new(|| {
		let mut m = HashMap::new();

		// Container-only boolean flag methods (0-arg)
		m.insert(("container", "h_expand"), BooleanAttrKind::FlagMethod);
		m.insert(("container", "w_expand"), BooleanAttrKind::FlagMethod);
		m.insert(("container", "w_fit"), BooleanAttrKind::FlagMethod);
		m.insert(("container", "center"), BooleanAttrKind::FlagMethod);
		m.insert(("container", "focusable"), BooleanAttrKind::FlagMethod);
		m.insert(
			("container", "focus_container"),
			BooleanAttrKind::FlagMethod,
		);

		// Text-only boolean flag methods (0-arg)
		m.insert(("text", "text_center"), BooleanAttrKind::FlagMethod);
		m.insert(("text", "text_right"), BooleanAttrKind::FlagMethod);
		m.insert(("text", "text_left"), BooleanAttrKind::FlagMethod);

		// Container-only bool-parameter toggle methods
		m.insert(("container", "floating"), BooleanAttrKind::ToggleBoolParam);

		m
	});

impl CodeGenerator {
	fn new() -> Self {
		Self
	}

	/// Generate Rust tokens for a DOM node.
	fn generate(&self, node: &Node) -> Result<TokenStream2, syn::Error> {
		self.generate_with_box(node, true)
	}

	/// Generate Rust tokens for a DOM node, with option to wrap in `Box::new(...)`.
	fn generate_with_box(&self, node: &Node, wrap_in_box: bool) -> Result<TokenStream2, syn::Error> {
		let code = match node {
			Node::Element(element) => self.generate_element_inner(element)?,
			Node::Text(text) => {
				let lit = syn::LitStr::new(&text.value, text.span);
				quote! { ardos_ui::Text::new(#lit) }
			}
			Node::Expression(expr) => {
				let parsed: syn::Expr = syn::parse2(expr.value.clone())
					.or_else(|_| {
						let ts = expr.value.clone();
						syn::parse2::<syn::Expr>(quote! { ( #ts ) })
					})
					.map_err(|e| syn::Error::new(expr.span, e.to_string()))?;
				quote! { #parsed }
			}
		};

		if wrap_in_box && matches!(node, Node::Element(_)) {
			Ok(quote! { (Box::new(#code) as Box<dyn ardos_ui::Element>) })
		} else {
			Ok(code)
		}
	}

	/// Generate Rust code for an RSML element.
	///
	/// Determines whether the element is a component (uppercase) or
	/// a built-in element (lowercase) and generates appropriate code.
	fn generate_element_inner(&self, element: &Element) -> Result<TokenStream2, syn::Error> {
		let tag_name = element.tag_name.value.as_str();
		let tag_span = element.tag_name.span;

		// Components start with uppercase letters
		if tag_name.chars().next().unwrap_or('a').is_uppercase() {
			return self.generate_component(element);
		}

		// Map RSML tag names to Ardos UI types
		let element_type: TokenStream2 = match tag_name {
			"container" => quote! { ardos_ui::Container },
			"image" => quote! { ardos_ui::Image },
			"text" => quote! { ardos_ui::Text },
			_ => {
				let ident = Ident::new(tag_name, tag_span);
				quote! { #ident }
			}
		};

		let mut code: TokenStream2 = if tag_name == "text" {
			// Text has special constructor: Text::new(content) or Text::new(format!(...))
			//
			// For `<text>...</text>`, apply HTML/JSX whitespace collapsing to plain text
			// children only (expressions are left intact).
			let mut fmt_parts: Vec<String> = Vec::new();
			let mut fmt_args: Vec<syn::Expr> = Vec::new();

			for child in &element.children {
				match child {
					Node::Text(t) => {
						let collapsed = collapse_html_whitespace(&t.value);
						if !collapsed.is_empty() {
							fmt_parts.push(collapsed);
						}
					}
					Node::Expression(e) => {
						fmt_parts.push("{}".to_string());

						// Wrap in identity macro to help tooling, then parse as an expression.
						let wrapped_ts = wrap_rsml_expr(e.value.clone());
						let parsed: syn::Expr = syn::parse2(wrapped_ts.clone())
							.or_else(|_| syn::parse2::<syn::Expr>(quote! { ( #wrapped_ts ) }))
							.map_err(|err| {
								syn::Error::new(
									e.span,
									format!(
										"Invalid Rust expression inside `<text>{{...}}</text>`: {}\nExpression was:\n{}",
										err,
										e.value.to_string()
									),
								)
							})?;
						fmt_args.push(parsed);
					}
					Node::Element(other) => {
						return Err(syn::Error::new(
							other.tag_name.span,
							"Text element cannot contain other elements",
						));
					}
				}
			}

			let format_string = fmt_parts.join(" ");
			let lit = syn::LitStr::new(&format_string, tag_span);

			if fmt_args.is_empty() {
				quote! { #element_type ::new(#lit) }
			} else {
				quote! { #element_type ::new(format!(#lit, #( #fmt_args ),*)) }
			}
		} else {
			quote! { #element_type ::new() }
		};

		// Convert attributes to method calls
		for attr in &element.attributes {
			let attr_name_str = attr.name.value.as_str();
			let attr_span = attr.name.span;
			let method_ident = Ident::new(attr_name_str, attr_span);

			match &attr.value {
				Some(AttributeValue::String(s)) => {
					let lit = syn::LitStr::new(&s.value, s.span);
					code = quote! { #code . #method_ident ( #lit ) };
				}
				Some(AttributeValue::Expression(e)) => {
					// Parse from token stream to preserve spans inside `{ ... }`.
					// Also support comma-separated `{a, b}` by retrying as a tuple `(a, b)`.
					//
					// Additionally, wrap in identity macro to help tooling.
					let wrapped_ts = wrap_rsml_expr(e.value.clone());
					let expr: syn::Expr = syn::parse2(wrapped_ts.clone())
						.or_else(|_| syn::parse2::<syn::Expr>(quote! { ( #wrapped_ts ) }))
						.map_err(|err| {
							syn::Error::new(
								e.span,
								format!(
									"Invalid Rust expression in attribute `{}` on `<{}>`: {}\nExpression was:\n{}",
									attr_name_str,
									tag_name,
									err,
									e.value.to_string()
								),
							)
						})?;

					if let Some(kind) = BOOLEAN_ATTR_RULES.get(&(tag_name, attr_name_str)).copied() {
						match kind {
							BooleanAttrKind::FlagMethod => {
								// Ensure condition expression is wrapped as a "bool-like" identity macro.
								let cond_ts = wrap_rsml_bool(e.value.clone());
								let cond: syn::Expr = syn::parse2(cond_ts.clone())
									.or_else(|_| syn::parse2::<syn::Expr>(quote! { ( #cond_ts ) }))
									.map_err(|err| syn::Error::new(e.span, err.to_string()))?;

								code = quote! { if #cond { #code . #method_ident () } else { #code } };
							}
							BooleanAttrKind::ToggleBoolParam => {
								code = quote! { #code . #method_ident ( #expr ) };
							}
						}
					} else {
						code = quote! { #code . #method_ident ( #expr ) };
					}
				}
				None => {
					if let Some(kind) = BOOLEAN_ATTR_RULES.get(&(tag_name, attr_name_str)).copied() {
						match kind {
							BooleanAttrKind::FlagMethod => {
								code = quote! { #code . #method_ident () };
							}
							BooleanAttrKind::ToggleBoolParam => {
								let t: syn::Expr = syn::parse_str("true").unwrap();
								code = quote! { #code . #method_ident ( #t ) };
							}
						}
					} else {
						code = quote! { #code . #method_ident () };
					}
				}
			}
		}

		// Add children as `.child(...)` calls (except for text which handles children differently)
		//
		// Strict JSX semantics:
		// - Only `<text>` may contain non-whitespace text nodes.
		// - For other elements (e.g. `<container>`), non-whitespace text nodes are an error and must be wrapped in `<text>...</text>`.
		if tag_name != "text" {
			for child in &element.children {
				match child {
					Node::Text(text) => {
						if text.value.trim().is_empty() {
							continue;
						}
						return Err(syn::Error::new(
							text.span,
							format!(
								"`<{tag_name}>` cannot contain raw text. Wrap it in `<text>...</text>` instead."
							),
						));
					}
					_ => {
						let child_code = self.generate_with_box(child, false)?;
						code = quote! { #code .child(#child_code) };
					}
				}
			}
		}

		Ok(code)
	}

	/// Generate Rust code for a component (uppercase tag).
	///
	/// Components are generated as Component::new(ComponentName, props)
	/// where props is built using the setup_props parameter:
	///
	/// ```rust,ignore
	/// ardos_ui::Component::new(MyComponent, |props| {
	///     props.name = "value";
	///     props.active = true;
	///     props.children = vec![/* child elements */];
	/// })
	/// ```
	///
	/// This allows Rust to infer the correct props type from the component function signature.
	fn generate_component(&self, element: &Element) -> Result<TokenStream2, syn::Error> {
		let component_ident = Ident::new(&element.tag_name.value, element.tag_name.span);

		// Build `props` assignments as tokens
		let mut props_stmts: Vec<TokenStream2> = Vec::new();
		let mut key = Option::<syn::Expr>::None;
		for attr in &element.attributes {
			let field_ident = Ident::new(&attr.name.value, attr.name.span);

			let stmt = match &attr.value {
				Some(AttributeValue::String(s)) => {
					let lit = syn::LitStr::new(&s.value, s.span);
					if field_ident.to_string() == "key" {
						key = Some(syn::parse2(quote!(#lit)).unwrap());
						quote!()
					} else {
						quote! { props.#field_ident = (#lit).into(); }
					}
				}
				Some(AttributeValue::Expression(e)) => {
					// Parse from token stream to preserve spans; support tuple fallback.
					// Wrap in identity macro to help tooling.
					let wrapped_ts = wrap_rsml_expr(e.value.clone());
					let expr: syn::Expr = syn::parse2(wrapped_ts.clone())
						.or_else(|_| syn::parse2::<syn::Expr>(quote! { ( #wrapped_ts ) }))
						.map_err(|err| {
							syn::Error::new(
								e.span,
								format!(
									"Invalid Rust expression for prop `{}` on component `<{}>`: {}\nExpression was:\n{}",
									attr.name.value,
									element.tag_name.value,
									err,
									e.value.to_string()
								),
							)
						})?;
					if field_ident.to_string() == "key" {
						key = Some(expr);
						quote!()
					} else {
						quote! { props.#field_ident = (#expr).into(); }
					}
				}
				None => quote! { props.#field_ident = true.into(); },
			};

			props_stmts.push(stmt);
		}

		// Convert children to props.children vector
		if !element.children.is_empty() {
			let mut children_tokens: Vec<TokenStream2> = Vec::new();
			for child in &element.children {
				match child {
					Node::Text(text) if text.value.trim().is_empty() => continue,
					_ => {
						children_tokens.push(self.generate_with_box(child, true)?);
					}
				}
			}

			if !children_tokens.is_empty() {
				props_stmts.push(quote! { props.children = vec![ #( #children_tokens ),* ]; });
			}
		}

		if props_stmts.is_empty() {
			match key {
				Some(key) => {
					Ok(quote! { ardos_ui::Component::new_with_key(#component_ident, |_| {}, #key) })
				}
				None => Ok(quote! { ardos_ui::Component::new(#component_ident, |_| {}) }),
			}
		} else {
			match key {
				Some(key) => Ok(quote! {
					ardos_ui::Component::new_with_key(#component_ident, |props| {
						#( #props_stmts )*
					}, #key)
				}),
				None => Ok(quote! {
					ardos_ui::Component::new(#component_ident, |props| {
						#( #props_stmts )*
					})
				}),
			}
		}
	}
}

// ============================================================================
// PROC MACRO
// ============================================================================

/// The `rsml!` procedural macro for writing Ardos UI components with JSX-like syntax.
///
/// This macro transforms RSML (RuSt Markup Language) syntax into Ardos UI Rust code.
///
/// # Example
///
/// ```rust,ignore
/// use ardos_ui::rsml;
///
/// let element = rsml! {
///     <container padding_all={16} center on_click={|| println!("Clicked!")}>
///         <text font_size={18}>Hello, World!</text>
///         <text>Click me!</text>
///     </container>
/// };
/// ```
///
/// The above expands to:
///
/// ```rust,ignore
/// Box::new(ardos_ui::Container::new().padding_all(16).center()
///     .on_click(|| println!("Clicked!"))
///     .child(Box::new(ardos_ui::Text::new("Hello, World!").font_size(18)))
///     .child(Box::new(ardos_ui::Text::new("Click me!"))))
/// ```
#[proc_macro]
pub fn rsml(input: TokenStream) -> TokenStream {
	// Token-tree based parse for span-preserving diagnostics.
	//
	// This keeps Rust tokens inside `{ ... }` as real Rust token streams so `syn::parse2`
	// can produce properly-spanned errors.
	match rsml_from_token_trees(TokenStream2::from(input)) {
		Ok(tokens) => tokens.into(),
		Err(e) => e.to_compile_error().into(),
	}
}

fn rsml_from_token_trees(input: TokenStream2) -> Result<TokenStream2, syn::Error> {
	let mut lexer = TokenTreeTokenizer::new(input);
	let dom = lexer.parse()?;
	let generator = CodeGenerator::new();
	generator.generate(&dom)
}

struct TokenTreeTokenizer {
	tokens: Vec<TokenTree>,
	idx: usize,
	in_tag: bool,
}

impl TokenTreeTokenizer {
	fn new(input: TokenStream2) -> Self {
		Self {
			tokens: input.into_iter().collect(),
			idx: 0,
			in_tag: false,
		}
	}

	fn peek(&self) -> Option<&TokenTree> {
		self.tokens.get(self.idx)
	}

	fn bump(&mut self) -> Option<TokenTree> {
		let tt = self.tokens.get(self.idx).cloned();
		if tt.is_some() {
			self.idx += 1;
		}
		tt
	}

	fn next_rsml_token(&mut self) -> Token {
		loop {
			let Some(tt) = self.bump() else {
				return Token {
					kind: TokenKind::Eof,
					span: Span::call_site(),
					payload: None,
					expr_tokens: None,
				};
			};

			match tt {
				TokenTree::Punct(p) if p.as_char() == '<' => {
					if matches!(self.peek(), Some(TokenTree::Punct(n)) if n.as_char() == '/') {
						let _ = self.bump(); // consume '/'
						self.in_tag = true;
						return Token {
							kind: TokenKind::EndOpenTag,
							span: p.span(),
							payload: None,
							expr_tokens: None,
						};
					}
					self.in_tag = true;
					return Token {
						kind: TokenKind::OpenTag,
						span: p.span(),
						payload: None,
						expr_tokens: None,
					};
				}
				TokenTree::Punct(p) if p.as_char() == '>' => {
					self.in_tag = false;
					return Token {
						kind: TokenKind::CloseTag,
						span: p.span(),
						payload: None,
						expr_tokens: None,
					};
				}
				TokenTree::Punct(p) if p.as_char() == '/' => {
					if matches!(self.peek(), Some(TokenTree::Punct(n)) if n.as_char() == '>') {
						let _ = self.bump(); // consume '>'
						self.in_tag = false;
						return Token {
							kind: TokenKind::SelfCloseTag,
							span: p.span(),
							payload: None,
							expr_tokens: None,
						};
					}
					continue;
				}
				TokenTree::Punct(p) if p.as_char() == '=' => {
					return Token {
						kind: TokenKind::Equals,
						span: p.span(),
						payload: None,
						expr_tokens: None,
					};
				}
				TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
					return Token {
						kind: TokenKind::Expression,
						span: g.span(),
						payload: Some(g.stream().to_string()),
						expr_tokens: Some(g.stream()),
					};
				}
				TokenTree::Literal(lit) => {
					return Token {
						kind: TokenKind::StringLiteral,
						span: lit.span(),
						payload: Some(lit.to_string().trim_matches('"').to_string()),
						expr_tokens: None,
					};
				}
				TokenTree::Ident(ident) => {
					if self.in_tag {
						return Token {
							kind: TokenKind::Identifier,
							span: ident.span(),
							payload: Some(ident.to_string()),
							expr_tokens: None,
						};
					} else {
						return Token {
							kind: TokenKind::Text,
							span: ident.span(),
							payload: Some(ident.to_string()),
							expr_tokens: None,
						};
					}
				}
				TokenTree::Punct(p) => {
					if !self.in_tag {
						return Token {
							kind: TokenKind::Text,
							span: p.span(),
							payload: Some(p.as_char().to_string()),
							expr_tokens: None,
						};
					}
					continue;
				}
				TokenTree::Group(g) => {
					if !self.in_tag {
						return Token {
							kind: TokenKind::Text,
							span: g.span(),
							payload: Some(g.stream().to_string()),
							expr_tokens: None,
						};
					}
					continue;
				}
			}
		}
	}

	fn parse(&mut self) -> Result<Node, syn::Error> {
		let mut tokens: Vec<Token> = Vec::new();
		loop {
			let t = self.next_rsml_token();
			let kind = t.kind;
			tokens.push(t);
			if kind == TokenKind::Eof {
				break;
			}
		}

		let mut p = TokenVectorParser::new(tokens);
		p.parse()
	}
}

struct TokenVectorParser {
	tokens: Vec<Token>,
	idx: usize,
	current_token: Token,
}

impl TokenVectorParser {
	fn new(tokens: Vec<Token>) -> Self {
		let first = tokens.get(0).cloned().unwrap_or(Token {
			kind: TokenKind::Eof,
			span: Span::call_site(),
			payload: None,
			expr_tokens: None,
		});

		Self {
			tokens,
			idx: 0,
			current_token: first,
		}
	}

	fn advance(&mut self) {
		self.idx += 1;
		self.current_token = self.tokens.get(self.idx).cloned().unwrap_or(Token {
			kind: TokenKind::Eof,
			span: Span::call_site(),
			payload: None,
			expr_tokens: None,
		});
	}

	fn expect_token_kind(&mut self, expected: TokenKind) -> Result<(), syn::Error> {
		if self.current_token.kind == expected {
			self.advance();
			Ok(())
		} else {
			Err(syn::Error::new(
				self.current_token.span,
				format!(
					"Expected {:?}, found {:?} (payload={:?})",
					expected, self.current_token.kind, self.current_token.payload
				),
			))
		}
	}

	fn parse_attributes(&mut self) -> Result<Vec<Attribute>, syn::Error> {
		let mut attributes = Vec::new();

		while self.current_token.kind == TokenKind::Identifier {
			let name_span = self.current_token.span;
			let name = self
				.current_token
				.payload
				.clone()
				.ok_or_else(|| syn::Error::new(name_span, "Expected identifier"))?;

			let attr_name = Spanned::new(name, name_span);
			self.advance();

			let value = if self.current_token.kind == TokenKind::Equals {
				self.advance();

				match self.current_token.kind {
					TokenKind::StringLiteral => {
						let span = self.current_token.span;
						let s = self
							.current_token
							.payload
							.clone()
							.ok_or_else(|| syn::Error::new(span, "Expected string literal"))?;
						self.advance();
						Some(AttributeValue::String(Spanned::new(s, span)))
					}
					TokenKind::Expression => {
						let span = self.current_token.span;
						let ts = self
							.current_token
							.expr_tokens
							.clone()
							.ok_or_else(|| syn::Error::new(span, "Expected expression"))?;
						self.advance();
						Some(AttributeValue::Expression(Spanned::new(ts, span)))
					}
					_ => {
						return Err(syn::Error::new(
							self.current_token.span,
							format!(
								"Expected string literal or expression after =, found {:?} (payload={:?})",
								self.current_token.kind, self.current_token.payload
							),
						));
					}
				}
			} else {
				None
			};

			attributes.push(Attribute {
				name: attr_name,
				value,
			});
		}

		Ok(attributes)
	}

	fn parse_element(&mut self) -> Result<Node, syn::Error> {
		self.expect_token_kind(TokenKind::OpenTag)?;

		let tag_name = if self.current_token.kind == TokenKind::Identifier {
			let span = self.current_token.span;
			let name = self
				.current_token
				.payload
				.clone()
				.ok_or_else(|| syn::Error::new(span, "Expected tag name after <"))?;
			self.advance();
			Spanned::new(name, span)
		} else {
			return Err(syn::Error::new(
				self.current_token.span,
				format!(
					"Expected tag name after <, found {:?} (payload={:?})",
					self.current_token.kind, self.current_token.payload
				),
			));
		};

		let attributes = self.parse_attributes()?;

		let self_closing = self.current_token.kind == TokenKind::SelfCloseTag;
		if self_closing {
			self.advance();
			return Ok(Node::Element(Element {
				tag_name,
				attributes,
				children: vec![],
				self_closing: true,
			}));
		}

		self.expect_token_kind(TokenKind::CloseTag)?;

		let mut children = Vec::new();
		while self.current_token.kind != TokenKind::EndOpenTag {
			match self.current_token.kind {
				TokenKind::OpenTag => children.push(self.parse_element()?),
				TokenKind::Expression => {
					let span = self.current_token.span;
					let ts = self
						.current_token
						.expr_tokens
						.clone()
						.ok_or_else(|| syn::Error::new(span, "Expected expression"))?;
					children.push(Node::Expression(Spanned::new(ts, span)));
					self.advance();
				}
				TokenKind::Text => {
					let span = self.current_token.span;
					let text = self
						.current_token
						.payload
						.clone()
						.ok_or_else(|| syn::Error::new(span, "Expected text"))?;
					children.push(Node::Text(Spanned::new(text, span)));
					self.advance();
				}
				TokenKind::Eof => {
					return Err(syn::Error::new(
						self.current_token.span,
						format!(
							"Unexpected EOF while parsing <{}> (last token kind={:?}, payload={:?})",
							tag_name.value.as_str(),
							self.current_token.kind,
							self.current_token.payload
						),
					));
				}
				_ => self.advance(),
			}
		}

		self.expect_token_kind(TokenKind::EndOpenTag)?;

		if self.current_token.kind == TokenKind::Identifier {
			let span = self.current_token.span;
			let closing_name = self
				.current_token
				.payload
				.clone()
				.ok_or_else(|| syn::Error::new(span, "Expected tag name in closing tag"))?;

			if closing_name != tag_name.value {
				return Err(syn::Error::new(
					span,
					format!(
						"Mismatched closing tag: expected </{}>, found </{}>",
						tag_name.value, closing_name
					),
				));
			}

			self.advance();
		} else {
			return Err(syn::Error::new(
				self.current_token.span,
				format!(
					"Expected tag name in closing tag, found {:?} (payload={:?})",
					self.current_token.kind, self.current_token.payload
				),
			));
		}

		self.expect_token_kind(TokenKind::CloseTag)?;

		Ok(Node::Element(Element {
			tag_name,
			attributes,
			children,
			self_closing: false,
		}))
	}

	fn parse(&mut self) -> Result<Node, syn::Error> {
		self.parse_element()
	}
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
	use super::*;
	use std::fs;
	use std::panic;

	/// Test harness that processes all RSML test files.
	///
	/// This test harness:
	/// 1. Reads all `.rsml` files from the `rsml_tests/` directory
	/// 2. Parses each file using the RSML compiler pipeline
	/// 3. Reports success/failure for each file
	/// 4. Provides a summary of results
	///
	/// Panics are caught and reported as failures to prevent one bad
	/// file from stopping the entire test suite.
	#[test]
	fn test_all_rsml_files() {
		let inputs_dir = "rsml_tests";

		// Create inputs directory if it doesn't exist
		if !std::path::Path::new(inputs_dir).exists() {
			fs::create_dir(inputs_dir).expect("Failed to create inputs directory");
			println!("Created rsml_tests/ directory. Add your test files there.");
			return;
		}

		// Read all files in inputs directory
		let entries = match fs::read_dir(inputs_dir) {
			Ok(entries) => entries,
			Err(e) => {
				panic!("Failed to read inputs directory: {}", e);
			}
		};

		let mut total_files = 0;
		let mut passed_files = 0;

		// Process each file in the directory
		for entry in entries {
			let entry = match entry {
				Ok(entry) => entry,
				Err(e) => {
					eprintln!("Error reading directory entry: {}", e);
					continue;
				}
			};

			let path = entry.path();
			if path.is_file() {
				total_files += 1;
				let filename = path.file_name().unwrap().to_string_lossy();

				print!("Testing {}: ", filename);

				// Read the RSML file
				let source = match fs::read_to_string(&path) {
					Ok(source) => source,
					Err(e) => {
						println!("FAIL (couldn't read file: {})", e);
						continue;
					}
				};

				// Convert fixture source into a TokenStream first, to match proc-macro input shape.
				// Note: this does not preserve fine-grained spans (yet), but it exercises the same
				// token-to-string conversion path as the macro pipeline.
				let fixture_ts: proc_macro2::TokenStream = match source.parse() {
					Ok(ts) => ts,
					Err(e) => {
						println!("FAIL (couldn't parse fixture into TokenStream: {})", e);
						continue;
					}
				};

				// Parse with panic handling to prevent crashes
				let result = panic::catch_unwind(|| {
					// Run the full compiler pipeline using token-tree parsing: parse → generate
					let mut lexer = TokenTreeTokenizer::new(fixture_ts);
					match lexer.parse() {
						Ok(dom) => {
							let generator = CodeGenerator::new();
							generator.generate(&dom)
						}
						Err(e) => Err(e),
					}
				});

				// Report results
				match result {
					Ok(Ok(tokens)) => {
						println!("PASS");
						println!("  Output: {}", tokens);
						passed_files += 1;
					}
					Ok(Err(parse_error)) => {
						println!("FAIL (parse error)");
						println!("  Message: {}", parse_error);
						println!("  Debug: {:#?}", parse_error);
						println!(
							"  Spanned compile_error!: {}",
							parse_error.to_compile_error()
						);
					}
					Err(_) => {
						println!("FAIL (panic during parsing)");
					}
				}
				println!(); // Empty line for readability
			}
		}

		// Print summary
		if total_files == 0 {
			println!("No files found in rsml_tests/ directory");
		} else {
			println!("Results: {}/{} files passed", passed_files, total_files);
			if passed_files != total_files {
				panic!("Some RSML test files failed!");
			}
		}
	}

	#[test]
	fn test_debug_expression_handling() {
		// Test expression handling specifically
		let rsml_input = r#"<text>{format!("Count: {}", count)}</text>"#;

		let ts: proc_macro2::TokenStream = rsml_input
			.parse()
			.expect("fixture must parse to TokenStream");
		let rsml_input = ts.to_string();

		match TokenTreeTokenizer::new(ts).parse() {
			Ok(dom) => {
				let generator = CodeGenerator::new();
				match generator.generate(&dom) {
					Ok(tokens) => {
						println!("Expression test - Generated code: {}", tokens);
					}
					Err(e) => {
						println!("Expression test - Codegen error: {}", e);
					}
				}
			}
			Err(e) => {
				println!("Expression test - Parse error: {}", e);
			}
		}
	}

	#[test]
	fn test_debug_failing_rsml_fixtures() {
		use std::fs;

		let default_span = proc_macro2::Span::call_site();
		let generator = CodeGenerator::new();

		let fixtures = [
			"rsml_tests/05_complex_nested.rsml",
			"rsml_tests/09_api_validation.rsml",
		];

		for rel_path in fixtures {
			println!("\n=== Debug fixture: {} ===", rel_path);

			let source = match fs::read_to_string(rel_path) {
				Ok(s) => s,
				Err(e) => {
					println!("FAILED to read fixture: {e}");
					continue;
				}
			};

			let fixture_ts: proc_macro2::TokenStream = match source.parse() {
				Ok(ts) => ts,
				Err(e) => {
					println!("FAILED to parse fixture into TokenStream: {e}");
					continue;
				}
			};
			// Dump token-tree lexer output to help pinpoint the exact token that breaks parsing.
			println!("--- Tokens (token-tree lexer) ---");
			{
				let mut t = TokenTreeTokenizer::new(fixture_ts.clone());
				loop {
					let tok = t.next_rsml_token();
					println!("  kind={:?} payload={:?}", tok.kind, tok.payload);
					if tok.kind == TokenKind::Eof {
						break;
					}
				}
			}

			// Parse + codegen.
			println!("--- Parse + Codegen ---");
			let mut lexer = TokenTreeTokenizer::new(fixture_ts);
			match lexer.parse() {
				Ok(dom) => match generator.generate(&dom) {
					Ok(tokens) => {
						println!("PASS parse+codegen");
						println!("Generated: {}", tokens);
					}
					Err(e) => {
						println!("FAIL codegen");
						println!("  Message: {}", e);
						println!("  Debug: {:#?}", e);
						println!("  Spanned compile_error!: {}", e.to_compile_error());
					}
				},
				Err(e) => {
					println!("FAIL parse");
					println!("  Message: {}", e);
					println!("  Debug: {:#?}", e);
					println!("  Spanned compile_error!: {}", e.to_compile_error());
				}
			}
		}
	}
}
