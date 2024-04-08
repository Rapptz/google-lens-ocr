# Google Lens OCR

A simple command line tool to run an image through Google Lens' OCR. It's meant to work via e.g. [ShareX](https://getsharex.com/actions).

## Usage

This tool is deliberately simple, so it only has two modes: output to stdout and copy to clipboard.

Output to stdout:

```shell
$ google-lens-ocr image.png
Hello World
```

Copy to clipboard:

```shell
$ google-lens-ocr clipboard image.png
```

That's it.

## License

Apache 2.0
