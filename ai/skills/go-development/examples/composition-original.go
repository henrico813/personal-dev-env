package main

import (
    "fmt"
)

func main() {
    Yell(&Person{ Yeller: yeller("%s!!!\n") }, "Nooooo")
    Yell(&RoboticVoice{Yeller: twiceYeller("*** %s ***")}, "Oh no")
    Whisper(&Person{ Whisperer: whisperer("Sssssh! %s!\n")}, "...")
    Whisper(&RoboticVoice{ Whisperer: whisperer("Sssssh! %s!\n")}, "...")
}

type Yeller interface {
    Yell(message string)
}

func Yell(y Yeller, message string) {
    y.Yell(message)
}

type Whisperer interface {
    Whisper(message string)
}

func Whisper(w Whisperer, message string) {
    w.Whisper(message)
}

type Person struct {
    Yeller
    Whisperer
}

type RoboticVoice struct {
    Yeller
    Whisperer
}

func (voice *RoboticVoice) Yell(message string) {
    fmt.Printf("BEEP! ")
    voice.Yeller.Yell(message)
    fmt.Printf(" BOP!\n")
}

func (voice *RoboticVoice) Whisper(message string) {
    fmt.Printf("Error! Cannot whisper! %s\n", message)
}

type yeller string

func (y yeller) Yell(message string) {
    fmt.Printf(string(y), message)
}

type twiceYeller string

func (twice twiceYeller) Yell(message string) {
    fmt.Printf(string(twice+twice), message, message)
}

type whisperer string
func (w whisperer) Whisper(message string) {
    fmt.Printf(string(w), message)
}
