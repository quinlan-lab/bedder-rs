-- MySQL dump 10.13  Distrib 5.6.10, for Linux (x86_64)
--
-- Host: localhost    Database: hg38
-- ------------------------------------------------------
-- Server version	5.6.10

/*!40101 SET @OLD_CHARACTER_SET_CLIENT=@@CHARACTER_SET_CLIENT */;
/*!40101 SET @OLD_CHARACTER_SET_RESULTS=@@CHARACTER_SET_RESULTS */;
/*!40101 SET @OLD_COLLATION_CONNECTION=@@COLLATION_CONNECTION */;
/*!40101 SET NAMES utf8 */;
/*!40103 SET @OLD_TIME_ZONE=@@TIME_ZONE */;
/*!40103 SET TIME_ZONE='+00:00' */;
/*!40101 SET @OLD_SQL_MODE=@@SQL_MODE, SQL_MODE='' */;
/*!40111 SET @OLD_SQL_NOTES=@@SQL_NOTES, SQL_NOTES=0 */;

--
-- Table structure for table `genomicSuperDups`
--

DROP TABLE IF EXISTS `genomicSuperDups`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `genomicSuperDups` (
  `bin` smallint(6) NOT NULL,
  `chrom` varchar(255) NOT NULL,
  `chromStart` int(10) unsigned NOT NULL,
  `chromEnd` int(10) unsigned NOT NULL,
  `name` varchar(255) NOT NULL,
  `score` int(10) unsigned NOT NULL,
  `strand` char(1) NOT NULL,
  `otherChrom` varchar(255) NOT NULL,
  `otherStart` int(10) unsigned NOT NULL,
  `otherEnd` int(10) unsigned NOT NULL,
  `otherSize` int(10) unsigned NOT NULL,
  `uid` int(10) unsigned NOT NULL,
  `posBasesHit` int(10) unsigned NOT NULL,
  `testResult` varchar(255) NOT NULL,
  `verdict` varchar(255) NOT NULL,
  `chits` varchar(255) NOT NULL,
  `ccov` varchar(255) NOT NULL,
  `alignfile` varchar(255) NOT NULL,
  `alignL` int(10) unsigned NOT NULL,
  `indelN` int(10) unsigned NOT NULL,
  `indelS` int(10) unsigned NOT NULL,
  `alignB` int(10) unsigned NOT NULL,
  `matchB` int(10) unsigned NOT NULL,
  `mismatchB` int(10) unsigned NOT NULL,
  `transitionsB` int(10) unsigned NOT NULL,
  `transversionsB` int(10) unsigned NOT NULL,
  `fracMatch` float NOT NULL,
  `fracMatchIndel` float NOT NULL,
  `jcK` float NOT NULL,
  `k2K` float NOT NULL,
  KEY `name` (`name`(32)),
  KEY `chrom` (`chrom`(8),`bin`),
  KEY `chrom_2` (`chrom`(8),`chromStart`)
) ENGINE=MyISAM DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

/*!40103 SET TIME_ZONE=@OLD_TIME_ZONE */;

/*!40101 SET SQL_MODE=@OLD_SQL_MODE */;
/*!40101 SET CHARACTER_SET_CLIENT=@OLD_CHARACTER_SET_CLIENT */;
/*!40101 SET CHARACTER_SET_RESULTS=@OLD_CHARACTER_SET_RESULTS */;
/*!40101 SET COLLATION_CONNECTION=@OLD_COLLATION_CONNECTION */;
/*!40111 SET SQL_NOTES=@OLD_SQL_NOTES */;

-- Dump completed on 2014-10-19 12:48:09
