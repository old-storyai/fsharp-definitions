// @ts-check

const readline = require("readline")
const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: false,
})

/** @type {string[]} */
const lines = []

rl.on("line", function (line) {
  lines.push(line)
})

rl.on("close", function () {
  printFSharpOutput()
  process.exit()
})

const KEEP_NOTES = process.env.FSFY_KEEP_NOTES === "1"
const NOTES_RE = /^\s*\/\/ NOTE: /

function printFSharpOutput() {
  const WARNING_SUMMARY_RE = /^warning: \d+/
  const lastWarningIndex = lines.findIndex((ln) => WARNING_SUMMARY_RE.test(ln))
  const linesWithoutCargoWarnings = lines.slice(lastWarningIndex + 1).map((ln) => ln.trimRight())

  const fsharpDefinitionsRaw = linesWithoutCargoWarnings
  const SECTION_START_RE = /\(\* ♒︎ section\(([^\)]+)\) \*\)/
  const SECTION_END_RE = /\(\* ♒︎ section end\(([^\)]+)\) \*\)/
  // const SECTION_RE = /\(\* ♒︎ section\(([^\)]+)\) \*\)([\s\S]+?)\(\* ♒︎ section end\(\1\) \*\)/g

  /** @type {null | string} */
  let currentSection = null
  const currentSectionLines = []
  const outputSections = new Map()
  for (let i = 0; i < fsharpDefinitionsRaw.length; i++) {
    const currentLine = fsharpDefinitionsRaw[i]
    if (currentSection === null) {
      const matchSectionStart = currentLine.match(SECTION_START_RE)
      if (matchSectionStart != null) {
        currentSection = matchSectionStart[1]
        currentSectionLines.length = 0
        continue
      }
    } else {
      // currently in a section
      const matchSectionEnd = currentLine.match(SECTION_END_RE)
      if (matchSectionEnd != null) {
        const matchedEndSection = matchSectionEnd[1]
        console.assert(
          matchedEndSection === currentSection,
          `Unexpected section end (${matchedEndSection}) not matching current section (${currentSection})`,
        )

        const sectionSource = currentSectionLines.join("\n")
        const existingSectionSource = outputSections.get(currentSection)
        // message when adding conflicting section
        if (existingSectionSource && existingSectionSource !== sectionSource) {
          console.error(
            `Found conflicting source defintion for "${currentSection}"\n\nAlready had:\n-------\n${existingSectionSource}\n-------\nBut found alternative definiton:\n-------\n${sectionSource}\n-------`,
          )
        } else {
          outputSections.set(currentSection, sectionSource)
        }

        currentSection = null
        currentSectionLines.length = 0
        continue
      } else if (currentLine.length > 0) {
        if (KEEP_NOTES || !NOTES_RE.test(currentLine)) {
          // new content
          currentSectionLines.push(currentLine)
        }
      }
    }
  }

  const fs = require("fs")
  const outHeaderContent = fs.readFileSync(expectEnv("FSFY_MODELS_HEAD_FILE"), "utf8")
  console.log(outHeaderContent)

  const sectionNames = [...outputSections.keys()]
  sectionNames.sort()
  const sectionsString = sectionNames.map((name) => outputSections.get(name)).join("\n\n")

  const dependsOnOpenFSharpJson = sectionsString.includes("JsonUnion")
  const alreadyHasOpenFSharpJson = outHeaderContent.includes("open FSharp.Json")

  if (dependsOnOpenFSharpJson && !alreadyHasOpenFSharpJson) {
    console.log("open FSharp.Json\n")
  }

  console.log(sectionsString)
}

function expectEnv(name) {
  const value = process.env[name]
  if (!value) throw new Error(`Expected ${name} env var to be set`)
  return value
}
