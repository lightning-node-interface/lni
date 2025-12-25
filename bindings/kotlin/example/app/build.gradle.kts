plugins {
    kotlin("jvm")
    application
}

group = "com.lni.example"
version = "0.1.0"

dependencies {
    // JNA for native library loading
    implementation("net.java.dev.jna:jna:5.13.0")
    
    // Kotlin coroutines for async operations
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")
    
    // Testing
    testImplementation(kotlin("test"))
}

// Include the generated LNI bindings
kotlin {
    sourceSets {
        main {
            kotlin.srcDir("../../src/main/kotlin")
        }
    }
}

application {
    mainClass.set("com.lni.example.MainKt")
}

tasks.test {
    useJUnitPlatform()
}

java {
    toolchain {
        languageVersion.set(JavaLanguageVersion.of(17))
    }
}
