plugins {
    id 'java'
    id 'org.jetbrains.kotlin.jvm' version '1.8.0'
    id 'maven-publish'
}

group = 'org.auditledger'
version = '0.1.0'
description = 'Java/Kotlin SDK for the AuditLedger Soroban smart contract'
repositories {
    mavenCentral()
}

java {
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
}

dependencies {
    // HTTP Client
    implementation 'org.apache.httpcomponents.client5:httpclient5:5.6.3'
    implementation 'org.apache.httpcomponents.core5:httpcore5:5.4.3'
    implementation 'org.apache.httpcomponents.core5:httpcore5-h2:5.4.3'
    
    // JSON Processing
    implementation 'com.fasterxml.jackson.core:jackson-core:2.18.11'
    implementation 'com.fasterxml.jackson.core:jackson-databind:2.18.11'
    
    // Kotlin Support
    implementation "org.jetbrains.kotlin:kotlin-stdlib-jdk8"
    
    // Reactive Streams (Reactor)
    implementation 'io.projectreactor:reactor-core:3.5.0'
    
    // Spring Framework (optional)
    implementation 'org.springframework:spring-context:6.0.0'
    
    // Testing
    testImplementation 'org.junit.jupiter:junit-jupiter:5.9.0'
    testImplementation 'org.mockito:mockito-core:5.0.0'
    testImplementation 'org.springframework.boot:spring-boot-starter-test:3.0.0'
}

tasks.withTest {
    useJUnitPlatform()
}

// Publishing configuration
publishing {
    publications {
        mavenJava(MavenPublication) {
            from components.java

            pom {
                name = 'AuditLedger SDK'
                description = 'Java/Kotlin SDK for the AuditLedger Soroban smart contract'
                url = 'https://github.com/daddygokings-art/Decentralized-Audit-Transparency-Ledger'

                licenses {
                    license {
                        name = 'The MIT License'
                        url = 'https://opensource.org/licenses/MIT'
                    }
                }
                developers {
                    developer {
                        name = 'AuditLedger Contributors'
                        email = 'contributors@auditledger.example.com'
                    }
                }
            }
        }
    }
    
    repositories {
        maven {
            // Set credentials and URL for your repository here
            // For Sonatype OSSRH:
            // url = uri("https://oss.sonatype.org/service/local/staging/deploy/maven2")
            // credentials {
            //     username = findProperty("ossrhUsername") ?: ""
            //     password = findProperty("ossrhPassword") ?: ""
            // }
        }
    }
}