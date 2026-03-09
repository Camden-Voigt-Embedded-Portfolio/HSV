# An HSV display for Microbit V2

Author: Camden J. Voigt

This is a simple embedded program written in rust for the microbit v2. It allows a user to set and H S or V value (depending on the mode) using a potentiometer those values are then saved as and HSV value and converted to RGB before being displayed using and RGB led. Mode can be switched using the A and B buttons on the microbit and will be displayed on the microbits display. To display the right color using the RGB value on the led I use a simple hand rolled PWM using a timer interrupt.

## Building and Running

With probe-rs installed and a microbit v2 connected run to flash onto the microbit.
`cargo run --release`

## How the project went

I learned a lot on this project. I understood the basic idea of how to program interrupts but this made it clear how it worked. I hadn't really realized you could have multiple handlers for different interrupt types. The begging of this project was really wrapping my head around how to use these interrupts correctly and getting all of the pieces setup correctly for both the buttons and the timer. Next, was understanding the PWM. It took a while to get how everything was laid out and then even longer to implement. It as hard to use the schedule, ticks and next schedule all lined up. Finally, I had a bug that was doing things in the wrong order that took a long time to find.

## Example

See PHOTO.jpg to see the wiring I used.
